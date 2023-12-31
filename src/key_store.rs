use chia_bls::PublicKey;
mod synthetic_key_store;

pub use synthetic_key_store::*;

pub trait KeyStore: Send + Sync {
    fn next_derivation_index(&self) -> u32;
    fn derive_keys(&mut self, count: u32);
    fn public_key(&self, index: u32) -> PublicKey;

    fn derive_keys_until(&mut self, index: u32) {
        if index < self.next_derivation_index() {
            return;
        }
        self.derive_keys(index - self.next_derivation_index());
    }
}
