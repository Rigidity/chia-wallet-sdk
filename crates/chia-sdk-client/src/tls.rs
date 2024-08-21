use std::fs;

use chia_ssl::ChiaCertificate;
use native_tls::{Identity, TlsConnector};
use thiserror::Error;

/// An error that occurs while trying to load an SSL certificate.
#[derive(Debug, Error)]
pub enum SslError {
    /// When an IO error occurs.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// When an SSL error occurs.
    #[error("ssl error: {0}")]
    Ssl(#[from] chia_ssl::Error),

    /// When a TLS error occurs.
    #[error("native tls error: {0}")]
    NativeTls(#[from] native_tls::Error),
}

/// Loads an SSL certificate, or creates it if it doesn't exist already.
pub fn load_ssl_cert(cert_path: &str, key_path: &str) -> Result<ChiaCertificate, SslError> {
    fs::read_to_string(cert_path)
        .and_then(|cert| {
            fs::read_to_string(key_path).map(|key| ChiaCertificate {
                cert_pem: cert,
                key_pem: key,
            })
        })
        .or_else(|_| {
            let cert = ChiaCertificate::generate()?;
            fs::write(cert_path, &cert.cert_pem)?;
            fs::write(key_path, &cert.key_pem)?;
            Ok(cert)
        })
}

/// Creates a TLS connector from a certificate.
pub fn create_tls_connector(cert: &ChiaCertificate) -> Result<TlsConnector, native_tls::Error> {
    let identity = Identity::from_pkcs8(cert.cert_pem.as_bytes(), cert.key_pem.as_bytes())?;

    TlsConnector::builder()
        .identity(identity)
        .danger_accept_invalid_certs(true)
        .build()
}
