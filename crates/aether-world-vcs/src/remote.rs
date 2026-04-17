//! Federation remote seam.
//!
//! In Aether's Bet 5 plan, worlds federate across servers the same
//! way git repositories federate across hosts: you name a remote,
//! pull its branches into yours, and push your branches back.
//!
//! v1 intentionally ships only the API shape + types + a
//! [`NullTransport`] so the surface can be wired into the gateway
//! and reviewed. A real transport will land under `aether-federation`
//! in a follow-up.

use serde::{Deserialize, Serialize};

use crate::branch::Branch;
use crate::diff::Cid;
use crate::error::Result;

/// A federation remote — a named world host we can fetch from and
/// push to.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Remote {
    /// Local alias for the remote, e.g. `"mirror-east"`.
    pub name: String,
    /// Remote endpoint URL (scheme depends on the transport).
    pub url: String,
    /// Remote host public key for signature verification. Raw 32
    /// bytes (ed25519) encoded as a hex string.
    pub public_key_hex: String,
}

/// Result of a `fetch` call.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FetchResult {
    /// Branches advertised by the remote.
    pub branches: Vec<Branch>,
    /// Diff CIDs newly discovered during this fetch.
    pub new_diffs: Vec<Cid>,
}

/// Result of a `push` call.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PushResult {
    /// Diff CIDs accepted by the remote.
    pub accepted: Vec<Cid>,
    /// Diff CIDs the remote rejected (with a reason).
    pub rejected: Vec<(Cid, String)>,
}

/// Pluggable transport behind federation. Implementors decide the
/// wire protocol. The VCS treats the transport as opaque.
pub trait RemoteTransport {
    /// Fetch branches + new diffs from the remote.
    fn fetch(&mut self, remote: &Remote) -> Result<FetchResult>;
    /// Push a local branch's tip to the remote.
    fn push(&mut self, remote: &Remote, branch: &str) -> Result<PushResult>;
}

/// A no-op transport. `fetch` returns empty results; `push` returns
/// an empty accept list. Intended as a stub so callers can wire the
/// surface and unit-test it before federation is real.
#[derive(Debug, Default)]
pub struct NullTransport;

impl NullTransport {
    /// Create a new null transport.
    pub fn new() -> Self {
        Self
    }
}

impl RemoteTransport for NullTransport {
    fn fetch(&mut self, _remote: &Remote) -> Result<FetchResult> {
        Ok(FetchResult::default())
    }

    fn push(&mut self, _remote: &Remote, _branch: &str) -> Result<PushResult> {
        Ok(PushResult::default())
    }
}
