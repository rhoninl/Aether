//! Ed25519 signing + verification for diffs.
//!
//! We hand-roll against `ed25519-dalek` rather than reusing
//! `aether-security` because that crate's helpers are currently
//! symmetric-auth / JWT oriented. Once `aether-security` grows a
//! general signature helper we can move to it.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

use crate::diff::{canonical_cbor, Diff, SignedDiff};
use crate::error::{Result, VcsError};

/// Generate a fresh Ed25519 keypair using the OS RNG.
///
/// Returns `(signing_key, verifying_key)`. Primarily for tests and
/// CLI tooling; production keys should come from an external KMS.
pub fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let mut rng = rand_core::OsRng;
    let sk = SigningKey::generate(&mut rng);
    let vk = sk.verifying_key();
    (sk, vk)
}

/// Sign a diff with the given Ed25519 signing key, producing a
/// [`SignedDiff`] whose signature covers the canonical CBOR encoding.
pub fn sign_diff(diff: Diff, sk: &SigningKey) -> Result<SignedDiff> {
    let bytes = canonical_cbor(&diff)?;
    let sig: Signature = sk.sign(&bytes);
    let vk = sk.verifying_key();
    Ok(SignedDiff {
        diff,
        signature: sig.to_bytes().to_vec(),
        public_key: vk.to_bytes().to_vec(),
    })
}

/// Verify a [`SignedDiff`]. Returns `Ok(())` iff the signature is
/// valid over the canonical CBOR encoding of `signed.diff` using
/// `signed.public_key`.
pub fn verify_signed_diff(signed: &SignedDiff) -> Result<()> {
    let bytes = canonical_cbor(&signed.diff)?;
    let vk_bytes: [u8; 32] = signed
        .public_key
        .as_slice()
        .try_into()
        .map_err(|_| VcsError::BadSignature("public key must be 32 bytes".into()))?;
    let vk = VerifyingKey::from_bytes(&vk_bytes)
        .map_err(|e| VcsError::BadSignature(format!("invalid public key: {e}")))?;
    let sig_bytes: [u8; 64] = signed
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| VcsError::BadSignature("signature must be 64 bytes".into()))?;
    let sig = Signature::from_bytes(&sig_bytes);
    vk.verify(&bytes, &sig)
        .map_err(|e| VcsError::BadSignature(e.to_string()))
}
