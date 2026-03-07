use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLanguage {
    Rust,
    AssemblyScript,
    CCpp,
    GoTinyGo,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeFlavor {
    Jit,
    Aot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformFamily {
    LinuxX64,
    LinuxAArch64,
    WindowsX64,
    MacArm,
    Android,
    VisionOs,
    Ps5,
    Quest,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformRuntimeProfile {
    ServerAlwaysAot,
    ClientJit,
    ClientAotOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptArtifact {
    pub platform: PlatformFamily,
    pub runtime: RuntimeFlavor,
    pub sha256: [u8; 32],
    pub blob_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WAsmArtifactManifest {
    pub script_id: u64,
    pub canonical_wasm_sha256: [u8; 32],
    pub artifacts: BTreeMap<(PlatformFamily, RuntimeFlavor), ScriptArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilationProfile {
    pub server_profile: PlatformRuntimeProfile,
    pub client_profile: PlatformRuntimeProfile,
    pub language: ScriptLanguage,
}

impl CompilationProfile {
    pub fn policy_for(platform: PlatformFamily) -> PlatformRuntimeProfile {
        match platform {
            PlatformFamily::LinuxX64 | PlatformFamily::LinuxAArch64 => PlatformRuntimeProfile::ServerAlwaysAot,
            PlatformFamily::WindowsX64 | PlatformFamily::MacArm => PlatformRuntimeProfile::ClientJit,
            PlatformFamily::Android | PlatformFamily::VisionOs | PlatformFamily::Ps5 | PlatformFamily::Quest => {
                PlatformRuntimeProfile::ClientAotOnly
            }
            PlatformFamily::Generic => PlatformRuntimeProfile::ClientJit,
        }
    }
}

impl Default for WAsmArtifactManifest {
    fn default() -> Self {
        Self {
            script_id: 0,
            canonical_wasm_sha256: [0u8; 32],
            artifacts: BTreeMap::new(),
        }
    }
}

pub type PlatformRuntimePolicy = CompilationProfile;
