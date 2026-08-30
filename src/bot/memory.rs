use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::sync::Mutex;

use crate::db::models::{IncomingProposal, Proposal, ProposalMessage};

const ACTIVE_TTL: Duration = Duration::from_secs(600);

pub struct ProposalStore {
    next_id: AtomicU64,
    inner: Mutex<Inner>,
}

struct Inner {
    pending: VecDeque<Proposal>,
    active: HashMap<u64, (Proposal, Instant)>,
    rejected: HashMap<u64, Proposal>,
    seen: HashSet<(i64, i32)>,
}

impl ProposalStore {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            inner: Mutex::new(Inner {
                pending: VecDeque::new(),
                active: HashMap::new(),
                rejected: HashMap::new(),
                seen: HashSet::new(),
            }),
        }
    }

    pub async fn push(&self, msg: IncomingProposal) -> bool {
        let mut inner = self.inner.lock().await;
        if inner.seen.len() > 50_000 {
            inner.seen.clear();
        }
        if !inner.seen.insert((msg.chat_id, msg.telegram_message_id)) {
            return false;
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let group_id = msg.proposal_group_id.clone();
        let entry = ProposalMessage {
            id,
            sender_id: msg.sender_id,
            message_text: msg.message_text,
            media_type: msg.media_type,
            media_file_id: msg.media_file_id,
            media_group_id: msg.media_group_id,
            proposal_group_id: group_id.clone(),
            parent_message_id: msg.parent_message_id,
            created_at: Utc::now(),
        };

        if let Some(existing) = inner.pending.iter_mut().find(|p| p.group_id == group_id) {
            existing.messages.push(entry);
        } else {
            inner.pending.push_back(Proposal {
                group_id,
                messages: vec![entry],
            });
        }
        true
    }

    pub async fn pop_next(&self) -> Option<Proposal> {
        let mut inner = self.inner.lock().await;

        let now = Instant::now();
        let stale: Vec<u64> = inner
            .active
            .iter()
            .filter(|(_, (_, activated_at))| now.duration_since(*activated_at) > ACTIVE_TTL)
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            if let Some((proposal, _)) = inner.active.remove(&id) {
                inner.pending.push_back(proposal);
            }
        }

        let proposal = inner.pending.pop_front()?;
        inner
            .active
            .insert(proposal.first().id, (proposal.clone(), now));
        Some(proposal)
    }

    pub async fn take_active(&self, id: u64) -> Option<Proposal> {
        self.inner.lock().await.active.remove(&id).map(|(p, _)| p)
    }

    pub async fn requeue(&self, proposal: Proposal) {
        let mut inner = self.inner.lock().await;
        inner.pending.push_front(proposal);
    }

    pub async fn reject(&self, id: u64) -> Option<Proposal> {
        let mut inner = self.inner.lock().await;
        let (proposal, _) = inner.active.remove(&id)?;
        inner.rejected.insert(id, proposal.clone());
        Some(proposal)
    }

    pub async fn rejected_sender(&self, id: u64) -> Option<i64> {
        self.inner
            .lock()
            .await
            .rejected
            .get(&id)
            .map(|p| p.first().sender_id)
    }

    pub async fn take_rejected(&self, id: u64) -> Option<Proposal> {
        self.inner.lock().await.rejected.remove(&id)
    }

    pub async fn restore_rejected(&self, proposal: Proposal) {
        let mut inner = self.inner.lock().await;
        inner.rejected.insert(proposal.first().id, proposal);
    }

    pub async fn discard_rejected(&self, id: u64) {
        self.inner.lock().await.rejected.remove(&id);
    }

    pub async fn return_to_pending(&self, id: u64) -> Option<Proposal> {
        let mut inner = self.inner.lock().await;
        let proposal = inner.rejected.remove(&id)?;
        inner.pending.push_front(proposal.clone());
        Some(proposal)
    }
}
