//! Aether scripting primitives.
//!
//! The module implements policy objects used by the WASM runtime pipeline:
//! - per-script and world-level resource caps
//! - script rate limiting
//! - world-script scheduling with priority and aging
//! - API surface traits consumed by sandbox host integrations

pub mod api;
pub mod config;
mod rate_limit;
pub mod artifact;
pub mod scheduler;
pub mod visual;

pub use api::{AudioApi, AudioHandle, EntityApi, NetworkApi, PhysicsApi, ScriptApiError, ScriptApiResult, StorageApi, UIApi, Vec3};
pub use config::{
    ScriptResourceLimits, ScriptRuntimeLimits, WorldScriptLimits, DEFAULT_PER_SCRIPT_CPU_LIMIT,
    DEFAULT_PER_SCRIPT_ENTITY_SPAWNS_PER_SECOND, DEFAULT_PER_SCRIPT_MEMORY_BYTES,
    DEFAULT_PER_SCRIPT_NETWORK_RPCS_PER_SECOND, DEFAULT_PER_SCRIPT_STORAGE_WRITES_PER_SECOND,
};
pub use artifact::{CompilationProfile, PlatformRuntimePolicy, ScriptArtifact, ScriptLanguage, WAsmArtifactManifest};
pub use visual::{VisualScriptCompiler, VisualScriptCompileError, VisualScriptGraph, VisualScriptNode};
pub use rate_limit::RateLimiter;
pub use scheduler::{
    ScriptDescriptor, ScriptExecutionUsage, ScriptId, ScriptRuntime, ScriptState,
    TickUsageResult, WorldScriptScheduler, WorldTick,
};
