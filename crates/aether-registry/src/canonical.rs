//! Canonical wire boundary for `aether-registry`.
//!
//! World discovery records are keyed by the CID of the canonical
//! `WorldManifest` that produced them. The in-memory `WorldManifest` type
//! (in `crate::manifest`) is unchanged for intra-crate use; this module is
//! the only path through which records enter/leave the crate.

use std::collections::BTreeMap;

use aether_canonical_shim::{
    CanonicalCodec, Cid, SchemaError, WorldDiscoveryRecord as WireRecord, WorldStatus as WireStatus,
};
use tracing::{info, trace};

use crate::manifest::{WorldCategory, WorldManifest, WorldStatus};

/// CID-keyed world discovery index.
///
/// Replaces the old string-keyed registry where the identity was the
/// `world_id`. Now identity is `Cid` — two registrations with equal
/// canonical bytes collapse to the same entry.
#[derive(Debug, Default)]
pub struct CanonicalWorldIndex {
    records: BTreeMap<Cid, WireRecord>,
}

impl CanonicalWorldIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a world from its canonical bytes. The CID of those bytes
    /// becomes the record's primary key.
    pub fn register_from_canonical(
        &mut self,
        world_manifest_bytes: &[u8],
        category: &WorldCategory,
        featured: bool,
    ) -> Result<Cid, SchemaError> {
        let span = tracing::trace_span!("registry::register_from_canonical", bytes = world_manifest_bytes.len());
        let _enter = span.enter();

        let manifest = aether_canonical_shim::WorldManifest::from_canonical_bytes(world_manifest_bytes)?;
        let cid = Cid::sha256_of(world_manifest_bytes);
        let record = WireRecord {
            manifest_cid: cid.clone(),
            world_id: manifest.world_id,
            slug: manifest.slug,
            name: manifest.name,
            owner_id: manifest.owner_id,
            category: category_string(category),
            featured,
            status: manifest.status,
            max_players: manifest.max_players,
            version: manifest.version,
        };
        self.records.insert(cid.clone(), record);
        info!(%cid, "registered world");
        Ok(cid)
    }

    /// Register from the in-memory `WorldManifest`. This is the "same
    /// thing, different entrypoint" convenience for callers that already
    /// hold the Rust struct; it still goes through canonical bytes so the
    /// CID is computed identically.
    pub fn register_from_manifest(
        &mut self,
        manifest: &WorldManifest,
    ) -> Result<Cid, SchemaError> {
        let canonical = manifest_to_canonical_bytes(manifest)?;
        self.register_from_canonical(&canonical, &manifest.category, manifest.featured)
    }

    pub fn get(&self, cid: &Cid) -> Option<&WireRecord> {
        self.records.get(cid)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Cid, &WireRecord)> {
        self.records.iter()
    }

    /// Export a record as canonical bytes — the wire form used when a
    /// registry replicates to another node.
    pub fn export_canonical(&self, cid: &Cid) -> Result<Option<Vec<u8>>, SchemaError> {
        let Some(record) = self.records.get(cid) else {
            return Ok(None);
        };
        let bytes = record.to_canonical_bytes()?;
        trace!(%cid, out_bytes = bytes.len(), "exported discovery record");
        Ok(Some(bytes))
    }
}

fn category_string(c: &WorldCategory) -> String {
    match c {
        WorldCategory::Social => "social".into(),
        WorldCategory::PvE => "pve".into(),
        WorldCategory::PvP => "pvp".into(),
        WorldCategory::Simulation => "simulation".into(),
        WorldCategory::Creative => "creative".into(),
        WorldCategory::Sandbox => "sandbox".into(),
        WorldCategory::Other(s) => format!("other:{s}"),
    }
}

fn status_to_wire(s: &WorldStatus) -> WireStatus {
    match s {
        WorldStatus::Draft => WireStatus::Draft,
        WorldStatus::Published => WireStatus::Published,
        WorldStatus::Deprecated => WireStatus::Deprecated,
    }
}

/// Encode an in-memory `WorldManifest` as canonical bytes. This is the
/// only crate-boundary emit path for registry manifests.
pub fn manifest_to_canonical_bytes(m: &WorldManifest) -> Result<Vec<u8>, SchemaError> {
    let wire = aether_canonical_shim::WorldManifest {
        world_id: m.world_id.clone(),
        slug: m.slug.clone(),
        name: m.name.clone(),
        owner_id: m.owner_id,
        version: m.version,
        status: status_to_wire(&m.status),
        max_players: m.max_players,
        gravity: 0.0,
        tick_rate_hz: 0,
        environment_path: String::new(),
        terrain_manifest: String::new(),
        props_manifest: String::new(),
        spawn_points: Vec::new(),
        portals: Vec::new(),
        region_preference: m.region_preference.clone(),
    };
    wire.to_canonical_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> WorldManifest {
        WorldManifest {
            world_id: "w-1".into(),
            slug: "slug".into(),
            name: "W".into(),
            owner_id: 1,
            category: WorldCategory::Social,
            featured: true,
            max_players: 16,
            region_preference: vec!["us-west".into()],
            status: WorldStatus::Published,
            version: 1,
            portal: "aether://w-1".into(),
        }
    }

    #[test]
    fn register_produces_stable_cid() {
        let mut idx = CanonicalWorldIndex::new();
        let m = sample();
        let c1 = idx.register_from_manifest(&m).unwrap();
        let c2 = idx.register_from_manifest(&m).unwrap();
        assert_eq!(c1, c2);
        assert_eq!(idx.len(), 1);
        let record = idx.get(&c1).unwrap();
        assert_eq!(record.manifest_cid, c1);
    }

    #[test]
    fn export_roundtrip() {
        let mut idx = CanonicalWorldIndex::new();
        let cid = idx.register_from_manifest(&sample()).unwrap();
        let bytes = idx.export_canonical(&cid).unwrap().unwrap();
        let back = WireRecord::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(back.manifest_cid, cid);
    }
}
