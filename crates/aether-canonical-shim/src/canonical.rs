use tracing::trace;

use crate::content_address::Cid;
use crate::error::SchemaError;

/// Trait implemented by every canonical wire type.
///
/// Mirrors the trait shape expected from `aether-schemas`:
///
/// ```ignore
/// pub trait CanonicalCodec {
///     fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError>;
///     fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError> where Self: Sized;
///     fn cid(&self) -> Cid { Cid::sha256_of(&self.to_canonical_bytes().unwrap()) }
/// }
/// ```
///
/// A `trace` span fires at every serialize/deserialize site so migration
/// across schema versions is observable.
pub trait CanonicalCodec {
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError>;

    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError>
    where
        Self: Sized;

    /// Derive the content id from the canonical byte form.
    fn cid(&self) -> Cid {
        let bytes = self
            .to_canonical_bytes()
            .expect("canonical encode must be infallible for well-formed in-memory values");
        let cid = Cid::sha256_of(&bytes);
        trace!(target: "aether_canonical_shim::codec", cid = %cid, bytes = bytes.len(), "cid");
        cid
    }
}
