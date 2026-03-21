pub mod apk;
pub mod cross_compile;
pub mod manifest;

use std::process::Command;

use crate::config::BuildConfig;
use crate::toolchain;
use crate::{BuildError, BuildOutput};

/// Build the project for Quest 3.
pub fn build(config: &BuildConfig) -> Result<BuildOutput, BuildError> {
    println!("Building for Quest 3 (aarch64-linux-android)...");

    // Step 1: Validate Android toolchain
    println!("  Checking Android toolchain...");
    let tc = toolchain::detect_android_toolchain()?;
    toolchain::validate_rust_target()?;

    // Step 2: Cross-compile
    let so_path = cross_compile::compile(config, &tc)?;

    // Step 3: Generate AndroidManifest.xml
    println!("  Generating AndroidManifest.xml...");
    let manifest_path = manifest::generate(config)?;

    // Step 4: Package APK
    let apk_path = apk::package(config, &tc, &so_path, &manifest_path)?;

    // Step 5: Optional install via adb
    if config.install_after_build {
        install_apk(&apk_path)?;
    }

    Ok(BuildOutput {
        artifact_path: apk_path,
        target_description: format!("Quest 3 ({})", config.profile),
    })
}

/// Install APK to connected device via adb.
fn install_apk(apk_path: &std::path::Path) -> Result<(), BuildError> {
    println!("  Installing APK via adb...");

    let output = Command::new("adb")
        .args(["install", "-r"])
        .arg(apk_path)
        .output()
        .map_err(|e| BuildError::ApkPackagingFailed {
            step: "adb install".to_string(),
            stderr: format!(
                "failed to run adb: {e}. Ensure adb is in PATH and device is connected."
            ),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BuildError::ApkPackagingFailed {
            step: "adb install".to_string(),
            stderr: stderr.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("  {}", stdout.trim());
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::{BuildConfig, BuildProfile, BuildTarget};
    use std::path::PathBuf;

    #[test]
    fn quest_build_config_defaults() {
        let config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Quest,
            BuildProfile::Debug,
        );
        assert_eq!(config.target, BuildTarget::Quest);
        assert_eq!(config.profile, BuildProfile::Debug);
        assert!(!config.install_after_build);
    }

    #[test]
    fn quest_build_config_with_install() {
        let mut config = BuildConfig::new(
            PathBuf::from("/project"),
            BuildTarget::Quest,
            BuildProfile::Debug,
        );
        config.install_after_build = true;
        assert!(config.install_after_build);
    }
}
