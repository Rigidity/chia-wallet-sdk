mod secret_key_store;

use chia_bls::PublicKey;
pub use secret_key_store::*;

pub trait KeyStore {
    fn public_key(&self, index: u32) -> PublicKey;
}