//! Asset conversion, processing, and bundle packaging.

pub mod audio;
pub mod budget;
pub mod bundle;
pub mod bundle_writer;
pub mod compression;
pub mod gltf_import;
pub mod hash;
pub mod import;
pub mod lod;
pub mod manifest;
pub mod mesh;
pub mod texture;

// Re-export existing types for backwards compatibility
pub use bundle::{BundleFormat, BundleManifest, Dependency, LODTier};
pub use compression::{TextureFormat, TextureTranscode, VertexCompression};
pub use import::{FbxImport, GltfImport, ImportTask, ObjImport};
pub use lod::{AutoLodPolicy, MeshLodSpec, ProgressionRule};
pub use manifest::{BundleAssetRecord, BundleError, PipelineTaskState};

// Re-export new processing types
pub use audio::{AudioCodec, AudioEncoder, AudioInput, EncodedAudio, PassthroughEncoder};
pub use budget::{AssetBudget, AssetUsage, BudgetCategory, BudgetReport, BudgetViolation};
pub use bundle_writer::{
    AssetBundle, AssetBundleManifest, BundleEntry, BundleWriteError, BundleWriter, ManifestEntry,
    WrittenBundle,
};
pub use gltf_import::{
    GltfImportError, GltfImporter, ImportedMaterial, ImportedScene, ImportedTexture,
};
pub use hash::{ContentHasher, HashedAsset};
pub use mesh::{ImportedMesh, LodChain, LodLevel, MeshOptimizer, SimpleMeshOptimizer, Vertex};
pub use texture::{
    CompressedTexture, PassthroughCompressor, TextureCompressor, TextureError, TextureInput,
};
