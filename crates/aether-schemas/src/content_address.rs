//! Content addressing (task 72).
//!
//! A [`Cid`] is a content identifier: the SHA-256 of the canonical CBOR
//! encoding of an artifact, tagged with the schema version that produced it.
//! The tagging means that if the canonical encoding ever changes in a
//! backward-incompatible way, two documents with the same logical contents
//! but different schema versions produce different CIDs. This is the correct
//! behavior for a content-addressed build graph.
//!
//! Wire form: `cid:v{version}:{lowercase-hex}`.
//! Example: `cid:v1:9b74c9897bac770ffc029102a200c5de...`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{SchemaError, SchemaResult};
use crate::migration::SchemaVersion;

/// 32-byte SHA-256 digest tagged with the schema version it was computed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cid {
    pub version: SchemaVersion,
    pub digest: [u8; 32],
}

impl Cid {
    /// Hash `bytes` with SHA-256 and tag the result with `version`.
    pub fn from_bytes(version: SchemaVersion, bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let out = hasher.finalize();
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&out);
        Cid { version, digest }
    }

    /// Lowercase hex representation of the digest.
    pub fn hex(&self) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(64);
        for b in self.digest {
            let _ = write!(s, "{:02x}", b);
        }
        s
    }
}

fn from_hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cid:v{}:{}", self.version.as_u32(), self.hex())
    }
}

impl FromStr for Cid {
    type Err = SchemaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rest = s.strip_prefix("cid:v").ok_or_else(|| SchemaError::Parse {
            pointer: "/".into(),
            message: format!("missing `cid:v` prefix in {:?}", s),
            suggested_fix: "CIDs look like `cid:v1:<64 hex chars>`".into(),
        })?;
        let (ver_str, hex) = rest.split_once(':').ok_or_else(|| SchemaError::Parse {
            pointer: "/".into(),
            message: "missing `:` between version and digest".into(),
            suggested_fix: "CIDs look like `cid:v1:<64 hex chars>`".into(),
        })?;
        let version_num: u32 = ver_str.parse().map_err(|_| SchemaError::Parse {
            pointer: "/".into(),
            message: format!("version {:?} is not an integer", ver_str),
            suggested_fix: "use `cid:v1:...`".into(),
        })?;
        let version = SchemaVersion::from_u32(version_num)?;

        if hex.len() != 64 {
            return Err(SchemaError::Parse {
                pointer: "/".into(),
                message: format!("digest must be 64 hex chars, got {}", hex.len()),
                suggested_fix: "CIDs carry a SHA-256 digest (64 hex chars)".into(),
            });
        }
        let mut digest = [0u8; 32];
        let bytes = hex.as_bytes();
        for (i, slot) in digest.iter_mut().enumerate() {
            let invalid = |pos| SchemaError::Parse {
                pointer: "/".into(),
                message: format!("invalid hex char at position {}", pos),
                suggested_fix: "CIDs use lowercase hex chars [0-9a-f]".into(),
            };
            let hi = from_hex(bytes[2 * i]).ok_or_else(|| invalid(2 * i))?;
            let lo = from_hex(bytes[2 * i + 1]).ok_or_else(|| invalid(2 * i + 1))?;
            *slot = (hi << 4) | lo;
        }
        Ok(Cid { version, digest })
    }
}

impl Serialize for Cid {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Cid {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        Cid::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Cid {
    fn schema_name() -> String {
        "Cid".to_string()
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{InstanceType, Metadata, Schema, SchemaObject, SingleOrVec};
        let mut schema = SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
            ..Default::default()
        };
        schema.metadata = Some(Box::new(Metadata {
            description: Some(
                "Content identifier: `cid:v{version}:{64 lowercase hex chars}`.".into(),
            ),
            ..Default::default()
        }));
        schema
            .string
            .get_or_insert_with(Default::default)
            .pattern = Some(r"^cid:v\d+:[0-9a-f]{64}$".into());
        Schema::Object(schema)
    }
}

/// Any artifact that has a canonical binary form implements `ContentAddress`.
pub trait ContentAddress {
    /// The schema version tag for the resulting [`Cid`].
    fn schema_version(&self) -> SchemaVersion;

    /// The canonical bytes (deterministic CBOR).
    fn canonical_bytes(&self) -> SchemaResult<Vec<u8>>;

    /// Compute the CID over the canonical bytes.
    fn cid(&self) -> SchemaResult<Cid> {
        let bytes = self.canonical_bytes()?;
        Ok(Cid::from_bytes(self.schema_version(), &bytes))
    }

    /// Verify a given CID matches the computed one, returning an agent-readable
    /// error if not.
    fn verify_cid(&self, expected: &Cid) -> SchemaResult<()> {
        let actual = self.cid()?;
        if actual == *expected {
            Ok(())
        } else {
            Err(SchemaError::CidMismatch {
                pointer: "/".into(),
                expected: expected.to_string(),
                actual: actual.to_string(),
                suggested_fix:
                    "the artifact has changed since the CID was recorded; regenerate or revert"
                        .into(),
            })
        }
    }
}

/// Opt-in trait for types that expose their schema version. Useful for
/// downstream crates that want to implement [`ContentAddress`] trivially
/// against the canonical encoding.
pub trait SchemaVersioned {
    fn schema_version(&self) -> SchemaVersion;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_empty_is_well_known() {
        let cid = Cid::from_bytes(SchemaVersion::V1, b"");
        // Well-known SHA-256 of the empty string.
        assert_eq!(
            cid.hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn cid_display_parse_roundtrip() {
        let cid = Cid::from_bytes(SchemaVersion::V1, b"hello");
        let s = cid.to_string();
        let back: Cid = s.parse().unwrap();
        assert_eq!(cid, back);
    }

    #[test]
    fn cid_display_format_shape() {
        let cid = Cid::from_bytes(SchemaVersion::V1, b"hello");
        let s = cid.to_string();
        assert!(s.starts_with("cid:v1:"));
        assert_eq!(s.len(), "cid:v1:".len() + 64);
    }

    #[test]
    fn cid_rejects_bad_prefix() {
        assert!("blah:v1:00".parse::<Cid>().is_err());
        assert!("cid:1:00".parse::<Cid>().is_err());
        assert!("cid:v1:xyz".parse::<Cid>().is_err());
    }

    #[test]
    fn cid_serializes_as_string() {
        let cid = Cid::from_bytes(SchemaVersion::V1, b"abc");
        let json = serde_json::to_string(&cid).unwrap();
        assert!(json.starts_with("\"cid:v1:"));
    }
}
