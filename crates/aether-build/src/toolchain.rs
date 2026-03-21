use std::path::PathBuf;
use std::process::Command;

use crate::config::{self, QUEST_RUST_TARGET};
use crate::BuildError;

/// Resolved paths to all Android build tools needed for APK packaging.
#[derive(Debug, Clone)]
pub struct AndroidToolchain {
    pub ndk_path: PathBuf,
    pub sdk_path: PathBuf,
    pub aapt2_path: PathBuf,
    pub zipalign_path: PathBuf,
    pub apksigner_path: PathBuf,
    pub ndk_clang_path: PathBuf,
    pub ndk_ar_path: PathBuf,
}

/// Detect and validate the full Android toolchain.
pub fn detect_android_toolchain() -> Result<AndroidToolchain, BuildError> {
    let ndk_path = config::android_ndk_home()?;
    let sdk_path = config::android_sdk_home()?;

    if !ndk_path.is_dir() {
        return Err(BuildError::ToolchainNotFound {
            tool: "Android NDK".to_string(),
            hint: format!(
                "ANDROID_NDK_HOME points to '{}' which does not exist",
                ndk_path.display()
            ),
        });
    }

    if !sdk_path.is_dir() {
        return Err(BuildError::ToolchainNotFound {
            tool: "Android SDK".to_string(),
            hint: format!(
                "ANDROID_HOME points to '{}' which does not exist",
                sdk_path.display()
            ),
        });
    }

    let build_tools_dir = config::find_latest_build_tools(&sdk_path)?;
    let aapt2_path = find_tool_in_dir(&build_tools_dir, "aapt2")?;
    let zipalign_path = find_tool_in_dir(&build_tools_dir, "zipalign")?;
    let apksigner_path = find_tool_in_dir(&build_tools_dir, "apksigner")?;

    let host_tag = config::ndk_host_tag();
    let toolchain_bin = ndk_path
        .join("toolchains/llvm/prebuilt")
        .join(host_tag)
        .join("bin");

    let ndk_clang_path = find_ndk_clang(&toolchain_bin)?;
    let ndk_ar_path = find_tool_in_dir(&toolchain_bin, "llvm-ar")?;

    Ok(AndroidToolchain {
        ndk_path,
        sdk_path,
        aapt2_path,
        zipalign_path,
        apksigner_path,
        ndk_clang_path,
        ndk_ar_path,
    })
}

/// Check that the Rust target for Quest is installed.
pub fn validate_rust_target() -> Result<(), BuildError> {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map_err(|e| BuildError::ToolchainNotFound {
            tool: "rustup".to_string(),
            hint: format!("Failed to run rustup: {e}"),
        })?;

    let installed = String::from_utf8_lossy(&output.stdout);
    if installed
        .lines()
        .any(|line| line.trim() == QUEST_RUST_TARGET)
    {
        Ok(())
    } else {
        Err(BuildError::RustTargetNotInstalled {
            target: QUEST_RUST_TARGET.to_string(),
        })
    }
}

/// Find the NDK clang for the Android API level.
fn find_ndk_clang(toolchain_bin: &std::path::Path) -> Result<PathBuf, BuildError> {
    // Try versioned clang first (e.g., aarch64-linux-android29-clang)
    for api in (config::QUEST_MIN_SDK_VERSION..=35).rev() {
        let name = format!("aarch64-linux-android{api}-clang");
        let path = toolchain_bin.join(&name);
        if path.exists() {
            return Ok(path);
        }
    }

    // Fall back to unversioned clang
    let fallback = toolchain_bin.join("aarch64-linux-android-clang");
    if fallback.exists() {
        return Ok(fallback);
    }

    Err(BuildError::ToolchainNotFound {
        tool: "NDK clang".to_string(),
        hint: format!(
            "No aarch64-linux-android clang found in {}",
            toolchain_bin.display()
        ),
    })
}

/// Find a tool binary in a directory.
fn find_tool_in_dir(dir: &std::path::Path, name: &str) -> Result<PathBuf, BuildError> {
    let path = dir.join(name);
    if path.exists() {
        return Ok(path);
    }
    // Try with .exe extension on Windows
    if cfg!(target_os = "windows") {
        let path_exe = dir.join(format!("{name}.exe"));
        if path_exe.exists() {
            return Ok(path_exe);
        }
    }
    // Try .bat for apksigner on Windows
    if cfg!(target_os = "windows") && name == "apksigner" {
        let path_bat = dir.join(format!("{name}.bat"));
        if path_bat.exists() {
            return Ok(path_bat);
        }
    }
    Err(BuildError::ToolchainNotFound {
        tool: name.to_string(),
        hint: format!("'{}' not found in {}", name, dir.display()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn detect_ndk_missing_env_var() {
        // When env var is not set, should get clear error
        // Note: This test may pass or fail depending on env — we test the helper directly
        let result = config::android_ndk_home();
        if std::env::var("ANDROID_NDK_HOME").is_err() {
            assert!(result.is_err());
            match result.unwrap_err() {
                BuildError::ToolchainNotFound { tool, hint } => {
                    assert_eq!(tool, "Android NDK");
                    assert!(hint.contains("ANDROID_NDK_HOME"));
                }
                _ => panic!("expected ToolchainNotFound"),
            }
        }
    }

    #[test]
    fn detect_sdk_missing_env_var() {
        let result = config::android_sdk_home();
        if std::env::var("ANDROID_HOME").is_err() && std::env::var("ANDROID_SDK_ROOT").is_err() {
            assert!(result.is_err());
            match result.unwrap_err() {
                BuildError::ToolchainNotFound { tool, hint } => {
                    assert_eq!(tool, "Android SDK");
                    assert!(hint.contains("ANDROID_HOME"));
                }
                _ => panic!("expected ToolchainNotFound"),
            }
        }
    }

    #[test]
    fn find_tool_in_dir_found() {
        let tmp = TempDir::new().unwrap();
        let tool_path = tmp.path().join("aapt2");
        std::fs::write(&tool_path, "").unwrap();
        let result = find_tool_in_dir(tmp.path(), "aapt2").unwrap();
        assert_eq!(result, tool_path);
    }

    #[test]
    fn find_tool_in_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let err = find_tool_in_dir(tmp.path(), "aapt2").unwrap_err();
        match err {
            BuildError::ToolchainNotFound { tool, .. } => {
                assert_eq!(tool, "aapt2");
            }
            _ => panic!("expected ToolchainNotFound"),
        }
    }

    #[test]
    fn find_ndk_clang_with_versioned() {
        let tmp = TempDir::new().unwrap();
        let clang = tmp.path().join("aarch64-linux-android29-clang");
        std::fs::write(&clang, "").unwrap();
        let result = find_ndk_clang(tmp.path()).unwrap();
        assert!(result
            .to_string_lossy()
            .contains("aarch64-linux-android29-clang"));
    }

    #[test]
    fn find_ndk_clang_picks_highest_api() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("aarch64-linux-android29-clang"), "").unwrap();
        std::fs::write(tmp.path().join("aarch64-linux-android33-clang"), "").unwrap();
        let result = find_ndk_clang(tmp.path()).unwrap();
        assert!(result
            .to_string_lossy()
            .contains("aarch64-linux-android33-clang"));
    }

    #[test]
    fn find_ndk_clang_fallback_unversioned() {
        let tmp = TempDir::new().unwrap();
        let clang = tmp.path().join("aarch64-linux-android-clang");
        std::fs::write(&clang, "").unwrap();
        let result = find_ndk_clang(tmp.path()).unwrap();
        assert!(result
            .to_string_lossy()
            .contains("aarch64-linux-android-clang"));
    }

    #[test]
    fn find_ndk_clang_missing() {
        let tmp = TempDir::new().unwrap();
        let err = find_ndk_clang(tmp.path()).unwrap_err();
        match err {
            BuildError::ToolchainNotFound { tool, .. } => {
                assert_eq!(tool, "NDK clang");
            }
            _ => panic!("expected ToolchainNotFound"),
        }
    }
}
