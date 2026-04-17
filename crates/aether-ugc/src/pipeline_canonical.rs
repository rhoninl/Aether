//! Canonical UGC pipeline: Upload → Scan → Approve → Publish.
//!
//! Every state transition here operates on **content-addressed canonical
//! artifacts** (CIDs over canonical bytes). The in-memory descriptors stay
//! as Rust types; the wire/disk boundary and the pipeline identity are the
//! CID.
//!
//! This is the object that task 74's acceptance criterion pins on — the
//! integration test in `tests/canonical_pipeline.rs` drives this machine
//! end-to-end and asserts the CID is stable across all four states.

use std::collections::HashMap;

use aether_canonical_shim::{
    ArtifactEnvelope, ArtifactKind, CanonicalCodec, Cid, ContentAddress, SchemaError,
};
use thiserror::Error;
use tracing::{info, trace};

/// The four canonical pipeline states. Order is strict: you cannot skip
/// forward (e.g. Upload → Publish without Scan+Approve).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalPipelineState {
    Uploaded,
    Scanning,
    Approved,
    Published,
    Rejected,
}

/// One entry in the pipeline, keyed by the CID of its canonical body.
#[derive(Debug, Clone)]
pub struct CanonicalArtifact {
    /// Content identity: SHA-256 of the canonical bytes.
    pub cid: Cid,
    /// Size of the canonical bytes (not the envelope).
    pub size_bytes: u64,
    pub kind: ArtifactKind,
    pub state: CanonicalPipelineState,
    pub scan_reason: Option<String>,
}

impl CanonicalArtifact {
    pub fn content_address(&self) -> ContentAddress {
        ContentAddress::new(self.cid.clone(), self.size_bytes)
    }
}

#[derive(Debug, Error)]
pub enum CanonicalPipelineError {
    #[error("artifact not found: {0}")]
    NotFound(String),
    #[error("illegal state transition for {cid}: {from:?} → {to:?}")]
    IllegalTransition {
        cid: String,
        from: CanonicalPipelineState,
        to: CanonicalPipelineState,
    },
    #[error(transparent)]
    Schema(#[from] SchemaError),
}

/// Content-addressed UGC pipeline. Holds canonical artifact bodies keyed
/// by their CID.
#[derive(Debug, Default)]
pub struct CanonicalUgcPipeline {
    artifacts: HashMap<Cid, CanonicalArtifact>,
    bodies: HashMap<Cid, Vec<u8>>,
}

impl CanonicalUgcPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Upload step. Accepts canonical bytes, computes the CID, registers
    /// the artifact, and returns the CID used as identity through the rest
    /// of the pipeline.
    pub fn upload(&mut self, envelope_bytes: &[u8]) -> Result<Cid, CanonicalPipelineError> {
        let span = tracing::trace_span!("ugc::upload", bytes = envelope_bytes.len());
        let _enter = span.enter();

        // Validate the envelope decodes. We don't need the inner value
        // here — just that it parses as canonical.
        let envelope = ArtifactEnvelope::from_canonical_bytes(envelope_bytes)?;
        let cid = Cid::sha256_of(envelope_bytes);

        let artifact = CanonicalArtifact {
            cid: cid.clone(),
            size_bytes: envelope_bytes.len() as u64,
            kind: envelope.kind,
            state: CanonicalPipelineState::Uploaded,
            scan_reason: None,
        };

        self.bodies.insert(cid.clone(), envelope_bytes.to_vec());
        self.artifacts.insert(cid.clone(), artifact);
        info!(%cid, "artifact uploaded");
        Ok(cid)
    }

    /// Transition Uploaded → Scanning.
    pub fn scan(&mut self, cid: &Cid) -> Result<(), CanonicalPipelineError> {
        self.advance(cid, CanonicalPipelineState::Uploaded, CanonicalPipelineState::Scanning)?;

        // Re-verify CID: the body bytes must still hash to the key. This
        // is the "content-addressed" invariant.
        let bytes = self
            .bodies
            .get(cid)
            .ok_or_else(|| CanonicalPipelineError::NotFound(cid.to_string()))?;
        let actual = Cid::sha256_of(bytes);
        if &actual != cid {
            return Err(CanonicalPipelineError::Schema(SchemaError::CidMismatch {
                expected: cid.to_string(),
                actual: actual.to_string(),
            }));
        }
        trace!(%cid, "scan verified cid");
        Ok(())
    }

    /// Transition Scanning → Approved (clean scan) or Rejected.
    pub fn approve(&mut self, cid: &Cid) -> Result<(), CanonicalPipelineError> {
        self.advance(cid, CanonicalPipelineState::Scanning, CanonicalPipelineState::Approved)
    }

    pub fn reject(&mut self, cid: &Cid, reason: impl Into<String>) -> Result<(), CanonicalPipelineError> {
        let reason_string = reason.into();
        let artifact = self
            .artifacts
            .get_mut(cid)
            .ok_or_else(|| CanonicalPipelineError::NotFound(cid.to_string()))?;
        if !matches!(
            artifact.state,
            CanonicalPipelineState::Uploaded | CanonicalPipelineState::Scanning
        ) {
            return Err(CanonicalPipelineError::IllegalTransition {
                cid: cid.to_string(),
                from: artifact.state,
                to: CanonicalPipelineState::Rejected,
            });
        }
        artifact.state = CanonicalPipelineState::Rejected;
        artifact.scan_reason = Some(reason_string);
        info!(%cid, "artifact rejected");
        Ok(())
    }

    /// Transition Approved → Published. The CID is the identity on which
    /// the registry can key the world discovery record.
    pub fn publish(&mut self, cid: &Cid) -> Result<ContentAddress, CanonicalPipelineError> {
        self.advance(cid, CanonicalPipelineState::Approved, CanonicalPipelineState::Published)?;
        let artifact = self.artifacts.get(cid).expect("just advanced");
        info!(%cid, "artifact published");
        Ok(artifact.content_address())
    }

    pub fn get(&self, cid: &Cid) -> Option<&CanonicalArtifact> {
        self.artifacts.get(cid)
    }

    pub fn body(&self, cid: &Cid) -> Option<&[u8]> {
        self.bodies.get(cid).map(Vec::as_slice)
    }

    fn advance(
        &mut self,
        cid: &Cid,
        from: CanonicalPipelineState,
        to: CanonicalPipelineState,
    ) -> Result<(), CanonicalPipelineError> {
        let artifact = self
            .artifacts
            .get_mut(cid)
            .ok_or_else(|| CanonicalPipelineError::NotFound(cid.to_string()))?;
        if artifact.state != from {
            return Err(CanonicalPipelineError::IllegalTransition {
                cid: cid.to_string(),
                from: artifact.state,
                to,
            });
        }
        artifact.state = to;
        trace!(%cid, from = ?from, to = ?to, "pipeline transition");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(body: &[u8]) -> Vec<u8> {
        let env = ArtifactEnvelope {
            kind: ArtifactKind::AssetBundle,
            body: body.to_vec(),
        };
        env.to_canonical_bytes().unwrap()
    }

    #[test]
    fn upload_scan_approve_publish_roundtrip() {
        let mut pipe = CanonicalUgcPipeline::new();
        let bytes = envelope(b"hello");
        let cid = pipe.upload(&bytes).unwrap();
        assert_eq!(pipe.get(&cid).unwrap().state, CanonicalPipelineState::Uploaded);
        pipe.scan(&cid).unwrap();
        assert_eq!(pipe.get(&cid).unwrap().state, CanonicalPipelineState::Scanning);
        pipe.approve(&cid).unwrap();
        assert_eq!(pipe.get(&cid).unwrap().state, CanonicalPipelineState::Approved);
        let addr = pipe.publish(&cid).unwrap();
        assert_eq!(pipe.get(&cid).unwrap().state, CanonicalPipelineState::Published);
        assert_eq!(addr.cid, cid);
    }

    #[test]
    fn cannot_skip_states() {
        let mut pipe = CanonicalUgcPipeline::new();
        let bytes = envelope(b"x");
        let cid = pipe.upload(&bytes).unwrap();
        let err = pipe.approve(&cid).unwrap_err();
        assert!(matches!(err, CanonicalPipelineError::IllegalTransition { .. }));
    }

    #[test]
    fn reject_from_scanning() {
        let mut pipe = CanonicalUgcPipeline::new();
        let cid = pipe.upload(&envelope(b"y")).unwrap();
        pipe.scan(&cid).unwrap();
        pipe.reject(&cid, "malware").unwrap();
        assert_eq!(pipe.get(&cid).unwrap().state, CanonicalPipelineState::Rejected);
    }
}
