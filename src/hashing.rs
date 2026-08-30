use sha2::{Digest, Sha256};

pub fn hash_user_id(user_id: i64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_id.to_le_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::hash_user_id;

    #[test]
    fn hides_raw_id_and_is_stable() {
        let h = hash_user_id(6569505824);
        assert_eq!(h.len(), 64);
        assert!(!h.contains("6569505824"));
        assert_eq!(h, hash_user_id(6569505824));
        assert_ne!(h, hash_user_id(1));
    }
}
