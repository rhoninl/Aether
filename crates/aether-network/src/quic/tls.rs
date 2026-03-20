use std::sync::Arc;

use quinn::crypto::rustls::QuicServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Ensure the ring crypto provider is installed for rustls.
///
/// This is idempotent - subsequent calls are no-ops.
fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Errors that can occur during TLS setup.
#[derive(Debug)]
pub enum TlsError {
    /// Failed to generate a self-signed certificate.
    CertGeneration(String),
    /// Failed to read a certificate or key file.
    FileRead(String),
    /// Failed to parse certificate or key data.
    Parse(String),
    /// Failed to build TLS configuration.
    Config(String),
}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsError::CertGeneration(msg) => write!(f, "TLS cert generation failed: {msg}"),
            TlsError::FileRead(msg) => write!(f, "TLS file read failed: {msg}"),
            TlsError::Parse(msg) => write!(f, "TLS parse failed: {msg}"),
            TlsError::Config(msg) => write!(f, "TLS config failed: {msg}"),
        }
    }
}

impl std::error::Error for TlsError {}

/// A pair of TLS certificate chain and private key ready for use.
pub struct TlsCertPair {
    pub certs: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
}

impl std::fmt::Debug for TlsCertPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsCertPair")
            .field("certs_count", &self.certs.len())
            .field("key", &"<redacted>")
            .finish()
    }
}

/// Generate a self-signed certificate for development use.
///
/// The certificate is valid for "localhost" and IP 127.0.0.1.
pub fn generate_self_signed() -> Result<TlsCertPair, TlsError> {
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])
            .map_err(|e| TlsError::CertGeneration(e.to_string()))?;

    let cert_der = CertificateDer::from(cert.cert);
    let key_der = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

    Ok(TlsCertPair {
        certs: vec![cert_der],
        key: PrivateKeyDer::Pkcs8(key_der),
    })
}

/// Load a TLS certificate and key from PEM files.
pub fn load_from_files(cert_path: &str, key_path: &str) -> Result<TlsCertPair, TlsError> {
    let cert_pem = std::fs::read(cert_path)
        .map_err(|e| TlsError::FileRead(format!("cert {cert_path}: {e}")))?;
    let key_pem =
        std::fs::read(key_path).map_err(|e| TlsError::FileRead(format!("key {key_path}: {e}")))?;

    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut &cert_pem[..])
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Parse(format!("cert parse: {e}")))?;

    if certs.is_empty() {
        return Err(TlsError::Parse(
            "no certificates found in PEM file".to_string(),
        ));
    }

    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .map_err(|e| TlsError::Parse(format!("key parse: {e}")))?
        .ok_or_else(|| TlsError::Parse("no private key found in PEM file".to_string()))?;

    Ok(TlsCertPair { certs, key })
}

/// Build a Quinn server config from a TLS cert pair.
pub fn build_server_config(pair: &TlsCertPair) -> Result<quinn::ServerConfig, TlsError> {
    ensure_crypto_provider();
    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(pair.certs.clone(), pair.key.clone_key())
        .map_err(|e| TlsError::Config(format!("server TLS config: {e}")))?;

    server_crypto.alpn_protocols = vec![b"aether-v1".to_vec()];

    let quic_server_config = QuicServerConfig::try_from(server_crypto)
        .map_err(|e| TlsError::Config(format!("QUIC server config: {e}")))?;

    let server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_server_config));
    Ok(server_config)
}

/// Build a Quinn client config that trusts the provided server certificates.
///
/// For development, this trusts self-signed certificates.
pub fn build_client_config_dev() -> quinn::ClientConfig {
    ensure_crypto_provider();
    let mut crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();

    crypto.alpn_protocols = vec![b"aether-v1".to_vec()];

    let mut client_config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
            .expect("client QUIC crypto config"),
    ));

    let mut transport_config = quinn::TransportConfig::default();
    transport_config.max_concurrent_bidi_streams(super::config::MAX_CONCURRENT_BI_STREAMS.into());
    transport_config.max_concurrent_uni_streams(super::config::MAX_CONCURRENT_UNI_STREAMS.into());
    transport_config.datagram_receive_buffer_size(Some(65536));

    client_config.transport_config(Arc::new(transport_config));
    client_config
}

/// A certificate verifier that skips verification (development only).
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_signed_cert_generation_succeeds() {
        let pair = generate_self_signed().expect("should generate self-signed cert");
        assert!(!pair.certs.is_empty(), "should have at least one cert");
    }

    #[test]
    fn server_config_builds_from_self_signed() {
        let pair = generate_self_signed().expect("cert gen");
        let config = build_server_config(&pair);
        assert!(config.is_ok(), "server config should build successfully");
    }

    #[test]
    fn client_dev_config_builds() {
        let config = build_client_config_dev();
        // Just verify it doesn't panic and produces a config
        let _ = config;
    }

    #[test]
    fn load_from_nonexistent_files_returns_error() {
        let result = load_from_files("/nonexistent/cert.pem", "/nonexistent/key.pem");
        assert!(result.is_err());
        match result.unwrap_err() {
            TlsError::FileRead(_) => {}
            other => panic!("expected FileRead error, got: {other:?}"),
        }
    }
}
