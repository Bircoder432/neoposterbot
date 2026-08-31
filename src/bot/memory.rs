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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::IncomingProposal;

    fn make_proposal(chat_id: i64, msg_id: i32, group_id: &str) -> IncomingProposal {
        IncomingProposal {
            chat_id,
            telegram_message_id: msg_id,
            sender_id: 123,
            message_text: "test text".into(),
            media_type: "photo".into(),
            media_file_id: "file_123".into(),
            media_group_id: if group_id.starts_with("mg_") {
                Some(group_id.to_string())
            } else {
                None
            },
            proposal_group_id: group_id.to_string(),
            parent_message_id: None,
        }
    }

    #[tokio::test]
    async fn test_push_and_pop_single() {
        let store = ProposalStore::new();
        let p = make_proposal(1, 10, "single_1");

        assert!(store.push(p).await);

        let popped = store.pop_next().await.unwrap();
        assert_eq!(popped.group_id, "single_1");
        assert_eq!(popped.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let store = ProposalStore::new();
        let p1 = make_proposal(1, 10, "single_1");
        let p2 = make_proposal(1, 10, "single_1");

        assert!(store.push(p1).await);
        assert!(!store.push(p2).await);

        let popped = store.pop_next().await.unwrap();
        assert_eq!(popped.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_media_group_aggregation() {
        let store = ProposalStore::new();
        let p1 = make_proposal(1, 10, "mg_album_1");
        let p2 = make_proposal(1, 11, "mg_album_1");
        let p3 = make_proposal(1, 12, "mg_album_1");

        store.push(p1).await;
        store.push(p2).await;
        store.push(p3).await;

        let popped = store.pop_next().await.unwrap();
        assert_eq!(popped.group_id, "mg_album_1");
        assert_eq!(popped.messages.len(), 3);
    }

    #[tokio::test]
    async fn test_reject_and_restore() {
        let store = ProposalStore::new();
        store.push(make_proposal(1, 10, "single_1")).await;

        let popped = store.pop_next().await.unwrap();
        let id = popped.first().id;

        let rejected = store.reject(id).await;
        assert!(rejected.is_some());

        assert!(store.pop_next().await.is_none());

        let restored = store.return_to_pending(id).await;
        assert!(restored.is_some());

        let repopped = store.pop_next().await.unwrap();
        assert_eq!(repopped.first().id, id);
    }
}
