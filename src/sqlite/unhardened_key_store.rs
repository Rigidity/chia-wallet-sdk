use chia_bls::{DerivableKey, PublicKey};
use chia_wallet::{standard::standard_puzzle_hash, DeriveSynthetic};
use sqlx::SqlitePool;

use crate::{KeyStore, PuzzleStore};

pub struct UnhardenedKeyStore {
    pool: SqlitePool,
    intermediate_pk: PublicKey,
    hidden_puzzle_hash: [u8; 32],
}

impl UnhardenedKeyStore {
    pub fn new(pool: SqlitePool, intermediate_pk: PublicKey, hidden_puzzle_hash: [u8; 32]) -> Self {
        Self {
            pool,
            intermediate_pk,
            hidden_puzzle_hash,
        }
    }

    pub async fn public_keys(&self) -> Vec<PublicKey> {
        sqlx::query!("SELECT `public_key` FROM `unhardened_keys`")
            .fetch_all(&self.pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| {
                let bytes = row.public_key.try_into().unwrap();
                PublicKey::from_bytes(&bytes).unwrap()
            })
            .collect()
    }
}

impl KeyStore for UnhardenedKeyStore {
    async fn count(&self) -> u32 {
        sqlx::query!("SELECT COUNT(*) AS `count` FROM `unhardened_keys`")
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .count as u32
    }

    async fn public_key(&self, index: u32) -> Option<PublicKey> {
        sqlx::query!(
            "SELECT `public_key` FROM `unhardened_keys` WHERE `index` = ?",
            index
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap()
        .map(|row| {
            let bytes = row.public_key.try_into().unwrap();
            PublicKey::from_bytes(&bytes).unwrap()
        })
    }

    async fn public_key_index(&self, public_key: &PublicKey) -> Option<u32> {
        let public_key = public_key.to_bytes().to_vec();
        sqlx::query!(
            "SELECT `index` FROM `unhardened_keys` WHERE `public_key` = ?",
            public_key
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap()
        .map(|row| row.index as u32)
    }

    async fn derive_to_index(&self, index: u32) {
        let mut tx = self.pool.begin().await.unwrap();

        let count = sqlx::query!("SELECT COUNT(*) AS `count` FROM `unhardened_keys`")
            .fetch_one(&self.pool)
            .await
            .unwrap()
            .count as u32;

        for i in count..index {
            let pk = self
                .intermediate_pk
                .derive_unhardened(i)
                .derive_synthetic(&self.hidden_puzzle_hash);
            let p2_puzzle_hash = standard_puzzle_hash(&pk);

            let pk_bytes = pk.to_bytes().to_vec();
            let p2_puzzle_hash_bytes = p2_puzzle_hash.to_vec();

            sqlx::query!(
                "
                INSERT INTO `unhardened_keys` (
                    `index`,
                    `public_key`,
                    `p2_puzzle_hash`
                )
                VALUES (?, ?, ?)
                ",
                i,
                pk_bytes,
                p2_puzzle_hash_bytes
            )
            .execute(&mut *tx)
            .await
            .unwrap();
        }

        tx.commit().await.unwrap();
    }
}

impl PuzzleStore for UnhardenedKeyStore {
    async fn puzzle_hash(&self, index: u32) -> Option<[u8; 32]> {
        sqlx::query!(
            "SELECT `p2_puzzle_hash` FROM `unhardened_keys` WHERE `index` = ?",
            index
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap()
        .map(|row| row.p2_puzzle_hash.try_into().unwrap())
    }

    async fn puzzle_hash_index(&self, puzzle_hash: [u8; 32]) -> Option<u32> {
        let puzzle_hash = puzzle_hash.to_vec();
        sqlx::query!(
            "SELECT `index` FROM `unhardened_keys` WHERE `p2_puzzle_hash` = ?",
            puzzle_hash
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap()
        .map(|row| row.index as u32)
    }

    async fn puzzle_hashes(&self) -> Vec<[u8; 32]> {
        sqlx::query!("SELECT `p2_puzzle_hash` FROM `unhardened_keys`")
            .fetch_all(&self.pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.p2_puzzle_hash.try_into().unwrap())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use chia_bls::{derive_keys::master_to_wallet_unhardened_intermediate, SecretKey};
    use chia_wallet::standard::DEFAULT_HIDDEN_PUZZLE_HASH;
    use hex::ToHex;

    use crate::testing::SEED;

    use super::*;

    #[sqlx::test]
    async fn test_key_pairs(pool: SqlitePool) {
        let root_pk = SecretKey::from_seed(SEED.as_ref()).public_key();
        let intermediate_pk = master_to_wallet_unhardened_intermediate(&root_pk);
        let key_store = UnhardenedKeyStore::new(pool, intermediate_pk, DEFAULT_HIDDEN_PUZZLE_HASH);

        // Derive the first 10 keys.
        key_store.derive_to_index(10).await;

        let pks_hex: Vec<String> = key_store
            .public_keys()
            .await
            .iter()
            .map(|pk| pk.to_bytes().encode_hex())
            .collect();

        let expected_pks_hex = vec![
            "8584adae5630842a1766bc444d2b872dd3080f4e5daaecf6f762a4be7dc148f37868149d4217f3dcc9183fe61e48d8bf",
            "b07c0a00a30501d18418df3ece3335d2c7339e0589e61b9230cffc9573d0df739726e84e55e91d68744b0f3791285b96",
            "963eea603ce281d63daca66f0926421f51d6d24027e498cb9d02f6477e3e01c4c4fda666fc3ea4199fdf566244ba74e0",
            "b33bbccea1926947b7a83080c8b6a193121bf3480411abeb5fb31fa70002c150ba1d40a5c6a53b36cdd51ea468f0c2e4",
            "a7bf25f67541a4e292a06282d714bbbc203a8bd6b0d0b804d097a071388f84665659a1a1f220130d97bcd2c4775f1077",
            "a8fa6e4e7732e36d6e4e537c172a2c1e7fd926a43abd191c5aa82974a54e9de1addb32ea404724722dedc78407bbb098",
            "b40b3c77251cea8e4c9cbbecbaa7fe40e9ad5e1298c83696d879cffd0c28f9ed61d5f3aec34eb44593861b8d8aba796e",
            "94e949fd1ea33ac4886511c39ee3b98d2580a6fd66d2bb8517de0a1cd0afefea29702b1f6a3e88e74ce0686c7d53bde8",
            "b042fccde247d98b363c6edb1d921da2b099493e00713ba8d44b3d777901f33b41dd496f58baff1c1fc725e3f16f4b13",
            "a67d7a1f2c0754f97f9db696fb95c9f5462eb0a3fcb60dc072aebfad1ff3faabb6dd8f769f37c2e4df01af81863e410c",
        ];
        assert_eq!(pks_hex, expected_pks_hex);
    }
}