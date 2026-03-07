#[derive(Debug, Clone)]
pub enum TlsMode {
    Disabled,
    TerminatedAtGateway,
    EndToEnd,
}

#[derive(Debug, Clone)]
pub struct TransportSecurityPolicy {
    pub transport: String,
    pub tls: TlsMode,
    pub require_client_cert: bool,
    pub enforce_hsts: bool,
}

