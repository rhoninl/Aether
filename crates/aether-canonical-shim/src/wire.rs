//! Canonical wire types.
//!
//! Each type implements [`CanonicalCodec`] so crates can
//! [`to_canonical_bytes`](CanonicalCodec::to_canonical_bytes) /
//! [`from_canonical_bytes`](CanonicalCodec::from_canonical_bytes) at their
//! boundary. The encoding is a small deterministic binary format:
//!
//! * 4-byte magic `AECB` (Aether Canonical Bytes)
//! * 2-byte big-endian schema version
//! * Fields written in a **fixed, documented order** (no named-key sort
//!   ambiguity). Each field is length-prefixed where variable-sized.
//!
//! The real `aether-schemas` crate uses deterministic CBOR; this shim's
//! format differs on the wire but provides the same guarantees required by
//! Aether's boundary crates: identical in-memory values produce identical
//! bytes, producing identical [`Cid`](crate::Cid)s.
//!
//! When `aether-schemas` lands this module is deleted and call sites point
//! at the real schemas instead.

use tracing::trace;

use crate::canonical::CanonicalCodec;
use crate::content_address::{Cid, ContentAddress};
use crate::error::SchemaError;

const MAGIC: [u8; 4] = *b"AECB";
const SCHEMA_V1: u16 = 1;

// ---- tiny deterministic encoder ---------------------------------------------

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(64),
        }
    }
    fn header(mut self, version: u16) -> Self {
        self.buf.extend_from_slice(&MAGIC);
        self.buf.extend_from_slice(&version.to_be_bytes());
        self
    }
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }
    fn f32(&mut self, v: f32) {
        // Use the IEEE-754 bit pattern for determinism (NaN canonicalized).
        let bits = if v.is_nan() { 0x7fc0_0000 } else { v.to_bits() };
        self.buf.extend_from_slice(&bits.to_be_bytes());
    }
    fn bool(&mut self, v: bool) {
        self.buf.push(u8::from(v));
    }
    fn str(&mut self, v: &str) {
        let bytes = v.as_bytes();
        self.u32(bytes.len() as u32);
        self.buf.extend_from_slice(bytes);
    }
    fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.buf.extend_from_slice(v);
    }
    fn finish(self) -> Vec<u8> {
        self.buf
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Result<Self, SchemaError> {
        if buf.len() < 6 || buf[..4] != MAGIC {
            return Err(SchemaError::Decode("bad magic".into()));
        }
        let version = u16::from_be_bytes([buf[4], buf[5]]);
        if version != SCHEMA_V1 {
            return Err(SchemaError::UnknownVersion(version));
        }
        Ok(Self { buf, pos: 6 })
    }
    fn need(&self, n: usize) -> Result<(), SchemaError> {
        if self.pos + n > self.buf.len() {
            return Err(SchemaError::Decode("truncated".into()));
        }
        Ok(())
    }
    fn u8(&mut self) -> Result<u8, SchemaError> {
        self.need(1)?;
        let v = self.buf[self.pos];
        self.pos += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16, SchemaError> {
        self.need(2)?;
        let v = u16::from_be_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }
    fn u32(&mut self) -> Result<u32, SchemaError> {
        self.need(4)?;
        let v = u32::from_be_bytes([
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }
    fn u64(&mut self) -> Result<u64, SchemaError> {
        self.need(8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.buf[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(u64::from_be_bytes(bytes))
    }
    fn f32(&mut self) -> Result<f32, SchemaError> {
        let bits = self.u32()?;
        Ok(f32::from_bits(bits))
    }
    fn bool(&mut self) -> Result<bool, SchemaError> {
        Ok(self.u8()? != 0)
    }
    fn str(&mut self) -> Result<String, SchemaError> {
        let len = self.u32()? as usize;
        self.need(len)?;
        let s = std::str::from_utf8(&self.buf[self.pos..self.pos + len])
            .map_err(|e| SchemaError::Decode(format!("utf8: {e}")))?
            .to_string();
        self.pos += len;
        Ok(s)
    }
    fn bytes(&mut self) -> Result<Vec<u8>, SchemaError> {
        let len = self.u32()? as usize;
        self.need(len)?;
        let v = self.buf[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(v)
    }
}

// ---- WorldManifest ----------------------------------------------------------

/// Canonical world manifest — the single source of truth at every boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldManifest {
    pub world_id: String,
    pub slug: String,
    pub name: String,
    pub owner_id: u64,
    pub version: u16,
    pub status: WorldStatus,
    pub max_players: u32,
    pub gravity: f32,
    pub tick_rate_hz: u32,
    pub environment_path: String,
    pub terrain_manifest: String,
    pub props_manifest: String,
    pub spawn_points: Vec<SpawnPointDef>,
    pub portals: Vec<PortalDef>,
    pub region_preference: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldStatus {
    Draft = 0,
    Published = 1,
    Deprecated = 2,
}

impl WorldStatus {
    fn from_u8(v: u8) -> Result<Self, SchemaError> {
        Ok(match v {
            0 => WorldStatus::Draft,
            1 => WorldStatus::Published,
            2 => WorldStatus::Deprecated,
            other => return Err(SchemaError::Decode(format!("bad WorldStatus {other}"))),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPointDef {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw_deg: f32,
    pub is_default: bool,
}

impl CanonicalCodec for WorldManifest {
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        let span = tracing::trace_span!("WorldManifest::to_canonical_bytes", world_id = %self.world_id);
        let _enter = span.enter();
        let mut w = Writer::new().header(SCHEMA_V1);
        w.str(&self.world_id);
        w.str(&self.slug);
        w.str(&self.name);
        w.u64(self.owner_id);
        w.u16(self.version);
        w.u8(self.status as u8);
        w.u32(self.max_players);
        w.f32(self.gravity);
        w.u32(self.tick_rate_hz);
        w.str(&self.environment_path);
        w.str(&self.terrain_manifest);
        w.str(&self.props_manifest);
        w.u32(self.spawn_points.len() as u32);
        for p in &self.spawn_points {
            w.u64(p.id);
            w.f32(p.x);
            w.f32(p.y);
            w.f32(p.z);
            w.f32(p.yaw_deg);
            w.bool(p.is_default);
        }
        w.u32(self.portals.len() as u32);
        for p in &self.portals {
            p.write_into(&mut w);
        }
        w.u32(self.region_preference.len() as u32);
        for r in &self.region_preference {
            w.str(r);
        }
        let bytes = w.finish();
        trace!(world_id = %self.world_id, bytes = bytes.len(), "encoded WorldManifest");
        Ok(bytes)
    }

    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        let span = tracing::trace_span!("WorldManifest::from_canonical_bytes", bytes = bytes.len());
        let _enter = span.enter();
        let mut r = Reader::new(bytes)?;
        let world_id = r.str()?;
        let slug = r.str()?;
        let name = r.str()?;
        let owner_id = r.u64()?;
        let version = r.u16()?;
        let status = WorldStatus::from_u8(r.u8()?)?;
        let max_players = r.u32()?;
        let gravity = r.f32()?;
        let tick_rate_hz = r.u32()?;
        let environment_path = r.str()?;
        let terrain_manifest = r.str()?;
        let props_manifest = r.str()?;
        let spawn_len = r.u32()? as usize;
        let mut spawn_points = Vec::with_capacity(spawn_len);
        for _ in 0..spawn_len {
            spawn_points.push(SpawnPointDef {
                id: r.u64()?,
                x: r.f32()?,
                y: r.f32()?,
                z: r.f32()?,
                yaw_deg: r.f32()?,
                is_default: r.bool()?,
            });
        }
        let portal_len = r.u32()? as usize;
        let mut portals = Vec::with_capacity(portal_len);
        for _ in 0..portal_len {
            portals.push(PortalDef::read_from(&mut r)?);
        }
        let region_len = r.u32()? as usize;
        let mut region_preference = Vec::with_capacity(region_len);
        for _ in 0..region_len {
            region_preference.push(r.str()?);
        }
        Ok(WorldManifest {
            world_id,
            slug,
            name,
            owner_id,
            version,
            status,
            max_players,
            gravity,
            tick_rate_hz,
            environment_path,
            terrain_manifest,
            props_manifest,
            spawn_points,
            portals,
            region_preference,
        })
    }
}

// ---- PortalDef --------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalScheme {
    Aether = 0,
    Https = 1,
    StaticWorld = 2,
}

impl PortalScheme {
    fn from_u8(v: u8) -> Result<Self, SchemaError> {
        Ok(match v {
            0 => PortalScheme::Aether,
            1 => PortalScheme::Https,
            2 => PortalScheme::StaticWorld,
            other => return Err(SchemaError::Decode(format!("bad PortalScheme {other}"))),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortalDef {
    pub scheme: PortalScheme,
    pub target: String,
    pub region: String,
    pub fallback: Option<String>,
}

impl PortalDef {
    fn write_into(&self, w: &mut Writer) {
        w.u8(self.scheme as u8);
        w.str(&self.target);
        w.str(&self.region);
        match &self.fallback {
            Some(f) => {
                w.bool(true);
                w.str(f);
            }
            None => w.bool(false),
        }
    }
    fn read_from(r: &mut Reader<'_>) -> Result<Self, SchemaError> {
        let scheme = PortalScheme::from_u8(r.u8()?)?;
        let target = r.str()?;
        let region = r.str()?;
        let fallback = if r.bool()? { Some(r.str()?) } else { None };
        Ok(PortalDef {
            scheme,
            target,
            region,
            fallback,
        })
    }
}

impl CanonicalCodec for PortalDef {
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        let mut w = Writer::new().header(SCHEMA_V1);
        self.write_into(&mut w);
        Ok(w.finish())
    }
    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        let mut r = Reader::new(bytes)?;
        PortalDef::read_from(&mut r)
    }
}

// ---- ChunkManifest ----------------------------------------------------------

/// Canonical pointer to a streamed world chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkManifest {
    pub world_id: String,
    pub chunks: Vec<ChunkRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkRef {
    pub chunk_id: u64,
    pub kind: u8, // 0=terrain 1=prop_mesh 2=lighting (matches `ChunkKind`)
    pub lod: u8,
    pub path: String,
    pub size_bytes: u64,
    pub content: ContentAddress,
}

impl CanonicalCodec for ChunkManifest {
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        let span = tracing::trace_span!("ChunkManifest::to_canonical_bytes", world_id = %self.world_id);
        let _enter = span.enter();
        let mut w = Writer::new().header(SCHEMA_V1);
        w.str(&self.world_id);
        w.u32(self.chunks.len() as u32);
        for c in &self.chunks {
            w.u64(c.chunk_id);
            w.u8(c.kind);
            w.u8(c.lod);
            w.str(&c.path);
            w.u64(c.size_bytes);
            w.str(c.content.cid.as_str());
            w.u64(c.content.size_bytes);
        }
        Ok(w.finish())
    }
    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        let span = tracing::trace_span!("ChunkManifest::from_canonical_bytes", bytes = bytes.len());
        let _enter = span.enter();
        let mut r = Reader::new(bytes)?;
        let world_id = r.str()?;
        let n = r.u32()? as usize;
        let mut chunks = Vec::with_capacity(n);
        for _ in 0..n {
            let chunk_id = r.u64()?;
            let kind = r.u8()?;
            let lod = r.u8()?;
            let path = r.str()?;
            let size_bytes = r.u64()?;
            let cid_str = r.str()?;
            let content_size = r.u64()?;
            chunks.push(ChunkRef {
                chunk_id,
                kind,
                lod,
                path,
                size_bytes,
                content: ContentAddress::new(Cid::from_string(cid_str), content_size),
            });
        }
        Ok(ChunkManifest { world_id, chunks })
    }
}

// ---- WorldDiscoveryRecord ---------------------------------------------------

/// Registry-facing canonical record for world discovery. The world is keyed
/// by the CID of its canonical [`WorldManifest`].
#[derive(Debug, Clone, PartialEq)]
pub struct WorldDiscoveryRecord {
    pub manifest_cid: Cid,
    pub world_id: String,
    pub slug: String,
    pub name: String,
    pub owner_id: u64,
    pub category: String,
    pub featured: bool,
    pub status: WorldStatus,
    pub max_players: u32,
    pub version: u16,
}

impl CanonicalCodec for WorldDiscoveryRecord {
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        let mut w = Writer::new().header(SCHEMA_V1);
        w.str(self.manifest_cid.as_str());
        w.str(&self.world_id);
        w.str(&self.slug);
        w.str(&self.name);
        w.u64(self.owner_id);
        w.str(&self.category);
        w.bool(self.featured);
        w.u8(self.status as u8);
        w.u32(self.max_players);
        w.u16(self.version);
        Ok(w.finish())
    }
    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        let mut r = Reader::new(bytes)?;
        let manifest_cid = Cid::from_string(r.str()?);
        let world_id = r.str()?;
        let slug = r.str()?;
        let name = r.str()?;
        let owner_id = r.u64()?;
        let category = r.str()?;
        let featured = r.bool()?;
        let status = WorldStatus::from_u8(r.u8()?)?;
        let max_players = r.u32()?;
        let version = r.u16()?;
        Ok(WorldDiscoveryRecord {
            manifest_cid,
            world_id,
            slug,
            name,
            owner_id,
            category,
            featured,
            status,
            max_players,
            version,
        })
    }
}

// ---- ArtifactEnvelope (UGC pipeline payload) --------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    WorldManifest = 0,
    ChunkManifest = 1,
    AssetBundle = 2,
    WorldScript = 3,
    AvatarModel = 4,
    VoicePack = 5,
    Unknown = 255,
}

impl ArtifactKind {
    fn from_u8(v: u8) -> Result<Self, SchemaError> {
        Ok(match v {
            0 => ArtifactKind::WorldManifest,
            1 => ArtifactKind::ChunkManifest,
            2 => ArtifactKind::AssetBundle,
            3 => ArtifactKind::WorldScript,
            4 => ArtifactKind::AvatarModel,
            5 => ArtifactKind::VoicePack,
            255 => ArtifactKind::Unknown,
            other => return Err(SchemaError::Decode(format!("bad ArtifactKind {other}"))),
        })
    }
}

/// Wrapper for any artifact flowing through the UGC pipeline. The body is
/// opaque canonical bytes (e.g. a canonical `WorldManifest`). The UGC
/// pipeline uses `ContentAddress::cid` as the stable identity through every
/// state transition.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactEnvelope {
    pub kind: ArtifactKind,
    pub body: Vec<u8>,
}

impl ArtifactEnvelope {
    /// Create an envelope carrying a canonical-encoded inner value.
    pub fn wrap<T: CanonicalCodec>(kind: ArtifactKind, value: &T) -> Result<Self, SchemaError> {
        Ok(ArtifactEnvelope {
            kind,
            body: value.to_canonical_bytes()?,
        })
    }

    pub fn content_address(&self) -> Result<ContentAddress, SchemaError> {
        let bytes = self.to_canonical_bytes()?;
        Ok(ContentAddress::new(Cid::sha256_of(&bytes), bytes.len() as u64))
    }

    /// CID of the inner body bytes (distinct from CID of the envelope).
    pub fn body_cid(&self) -> Cid {
        Cid::sha256_of(&self.body)
    }
}

impl CanonicalCodec for ArtifactEnvelope {
    fn to_canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        let mut w = Writer::new().header(SCHEMA_V1);
        w.u8(self.kind as u8);
        w.bytes(&self.body);
        Ok(w.finish())
    }
    fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        let mut r = Reader::new(bytes)?;
        let kind = ArtifactKind::from_u8(r.u8()?)?;
        let body = r.bytes()?;
        Ok(ArtifactEnvelope { kind, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> WorldManifest {
        WorldManifest {
            world_id: "world-1".into(),
            slug: "hello-world".into(),
            name: "Hello World".into(),
            owner_id: 42,
            version: 1,
            status: WorldStatus::Published,
            max_players: 32,
            gravity: -9.8,
            tick_rate_hz: 60,
            environment_path: "env/day".into(),
            terrain_manifest: "terrain.man".into(),
            props_manifest: "props.man".into(),
            spawn_points: vec![SpawnPointDef {
                id: 1,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw_deg: 0.0,
                is_default: true,
            }],
            portals: vec![PortalDef {
                scheme: PortalScheme::Aether,
                target: "other-world".into(),
                region: "us-west".into(),
                fallback: None,
            }],
            region_preference: vec!["us-west".into(), "us-east".into()],
        }
    }

    #[test]
    fn roundtrip_world_manifest() {
        let m = sample_manifest();
        let bytes = m.to_canonical_bytes().unwrap();
        let back = WorldManifest::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn cid_is_stable() {
        let m = sample_manifest();
        let c1 = m.cid();
        let c2 = m.clone().cid();
        assert_eq!(c1, c2);
        assert!(c1.as_str().starts_with("sha256:"));
    }

    #[test]
    fn cid_differs_on_change() {
        let mut m = sample_manifest();
        let c1 = m.cid();
        m.max_players = 33;
        let c2 = m.cid();
        assert_ne!(c1, c2);
    }

    #[test]
    fn chunk_manifest_roundtrip() {
        let cm = ChunkManifest {
            world_id: "w".into(),
            chunks: vec![ChunkRef {
                chunk_id: 1,
                kind: 0,
                lod: 2,
                path: "chunks/1.bin".into(),
                size_bytes: 4096,
                content: ContentAddress::new(Cid::sha256_of(b"hello"), 5),
            }],
        };
        let bytes = cm.to_canonical_bytes().unwrap();
        let back = ChunkManifest::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(cm, back);
    }

    #[test]
    fn envelope_wraps_manifest() {
        let m = sample_manifest();
        let env = ArtifactEnvelope::wrap(ArtifactKind::WorldManifest, &m).unwrap();
        let back = ArtifactEnvelope::from_canonical_bytes(&env.to_canonical_bytes().unwrap()).unwrap();
        assert_eq!(env, back);
        let inner = WorldManifest::from_canonical_bytes(&env.body).unwrap();
        assert_eq!(inner, m);
    }

    #[test]
    fn bad_magic_rejected() {
        let err = WorldManifest::from_canonical_bytes(b"not really canonical bytes").unwrap_err();
        assert!(matches!(err, SchemaError::Decode(_)));
    }
}
