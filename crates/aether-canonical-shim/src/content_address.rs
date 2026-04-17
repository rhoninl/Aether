use sha2::{Digest, Sha256};
use std::fmt;

/// SHA-256 content identifier. Format: `"sha256:<hex>"`.
///
/// `Cid` is the content-addressed identifier used everywhere a canonical
/// artifact crosses a crate boundary. Two artifacts with the same
/// canonical bytes MUST produce the same `Cid`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cid(String);

impl Cid {
    /// Compute the SHA-256 CID for a byte buffer.
    pub fn sha256_of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut out = String::with_capacity(7 + digest.len() * 2);
        out.push_str("sha256:");
        for byte in digest {
            use std::fmt::Write;
            let _ = write!(&mut out, "{byte:02x}");
        }
        Cid(out)
    }

    /// Wrap a pre-formatted string (e.g. when loaded from storage).
    pub fn from_string(s: impl Into<String>) -> Self {
        Cid(s.into())
    }

    /// Raw `"sha256:<hex>"` form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Content-addressed pointer: the CID plus the byte length of the artifact
/// it points at.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentAddress {
    pub cid: Cid,
    pub size_bytes: u64,
}

impl ContentAddress {
    pub fn new(cid: Cid, size_bytes: u64) -> Self {
        Self { cid, size_bytes }
    }
}
