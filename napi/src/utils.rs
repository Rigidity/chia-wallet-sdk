use clvmr::sha2::Sha256;
use napi::bindgen_prelude::*;

use crate::traits::IntoJs;

#[napi]
pub fn compare_bytes(a: Uint8Array, b: Uint8Array) -> bool {
    a.as_ref() == b.as_ref()
}

#[napi]
pub fn sha256(bytes: Uint8Array) -> Result<Uint8Array> {
    let mut hasher = Sha256::new();
    hasher.update(bytes.as_ref());
    hasher.finalize().into_js()
}

#[napi]
pub fn from_hex_raw(hex: String) -> Result<Uint8Array> {
    let bytes = hex::decode(hex).map_err(|error| Error::from_reason(error.to_string()))?;
    bytes.into_js()
}

#[napi]
pub fn from_hex(hex: String) -> Result<Uint8Array> {
    let mut hex = hex.as_str();

    if let Some(stripped) = hex.strip_prefix("0x") {
        hex = stripped;
    }

    let bytes = hex::decode(hex).map_err(|error| Error::from_reason(error.to_string()))?;
    bytes.into_js()
}

#[napi]
pub fn to_hex(bytes: Uint8Array) -> String {
    hex::encode(bytes.as_ref())
}