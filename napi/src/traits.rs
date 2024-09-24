use chia::{
    bls::PublicKey,
    protocol::{BytesImpl, Program},
};
use napi::bindgen_prelude::*;

pub(crate) trait IntoJs<T> {
    fn into_js(self) -> Result<T>;
}

pub(crate) trait FromJs<T> {
    fn from_js(js_value: T) -> Result<Self>
    where
        Self: Sized;
}

pub(crate) trait IntoRust<T> {
    fn into_rust(self) -> Result<T>;
}

impl<T, U> IntoRust<U> for T
where
    U: FromJs<T>,
{
    fn into_rust(self) -> Result<U> {
        U::from_js(self)
    }
}

impl<const N: usize> IntoJs<Uint8Array> for BytesImpl<N> {
    fn into_js(self) -> Result<Uint8Array> {
        Ok(Uint8Array::new(self.to_vec()))
    }
}

impl<const N: usize> FromJs<Uint8Array> for BytesImpl<N> {
    fn from_js(js_value: Uint8Array) -> Result<Self> {
        Ok(Self::new(js_value.to_vec().try_into().map_err(
            |bytes: Vec<u8>| {
                Error::from_reason(format!("Expected length {N}, found {}", bytes.len()))
            },
        )?))
    }
}

impl<const N: usize> IntoJs<Uint8Array> for [u8; N] {
    fn into_js(self) -> Result<Uint8Array> {
        Ok(Uint8Array::new(self.to_vec()))
    }
}

impl<const N: usize> FromJs<Uint8Array> for [u8; N] {
    fn from_js(js_value: Uint8Array) -> Result<Self> {
        js_value.to_vec().try_into().map_err(|bytes: Vec<u8>| {
            Error::from_reason(format!("Expected length {N}, found {}", bytes.len()))
        })
    }
}

impl IntoJs<Uint8Array> for PublicKey {
    fn into_js(self) -> Result<Uint8Array> {
        Ok(Uint8Array::new(self.to_bytes().to_vec()))
    }
}

impl FromJs<Uint8Array> for PublicKey {
    fn from_js(js_value: Uint8Array) -> Result<Self>
    where
        Self: Sized,
    {
        PublicKey::from_bytes(&js_value.into_rust()?)
            .map_err(|error| Error::from_reason(error.to_string()))
    }
}

impl IntoJs<Uint8Array> for Program {
    fn into_js(self) -> Result<Uint8Array> {
        Ok(Uint8Array::new(self.to_vec()))
    }
}

impl IntoJs<BigInt> for u64 {
    fn into_js(self) -> Result<BigInt> {
        Ok(BigInt::from(self))
    }
}

impl FromJs<BigInt> for u64 {
    fn from_js(js_value: BigInt) -> Result<Self> {
        let (signed, value, lossless) = js_value.get_u64();

        if signed || !lossless {
            return Err(Error::from_reason("Expected u64"));
        }

        Ok(value)
    }
}