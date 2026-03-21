use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{BuildConfig, BuildProfile};
use crate::toolchain::AndroidToolchain;
use crate::BuildError;

const UNSIGNED_APK_NAME: &str = "app-unsigned.apk";
const ALIGNED_APK_NAME: &str = "app-aligned.apk";

/// Package the compiled .so and manifest into an APK.
pub fn package(
    config: &BuildConfig,
    toolchain: &AndroidToolchain,
    so_path: &Path,
    manifest_path: &Path,
) -> Result<PathBuf, BuildError> {
    println!("  Packaging APK...");

    let build_dir = config.build_dir();
    let output_dir = config.output_dir();
    fs::create_dir_all(&output_dir).map_err(|e| BuildError::IoError {
        context: "creating output directory".to_string(),
        source: e,
    })?;

    // Step 1: Use aapt2 to link manifest into base APK
    let unsigned_apk = build_dir.join(UNSIGNED_APK_NAME);
    run_aapt2_link(toolchain, manifest_path, &unsigned_apk)?;

    // Step 2: Add the native library to the APK
    add_native_lib_to_apk(&unsigned_apk, so_path, &build_dir)?;

    // Step 3: zipalign
    let aligned_apk = build_dir.join(ALIGNED_APK_NAME);
    run_zipalign(toolchain, &unsigned_apk, &aligned_apk)?;

    // Step 4: Sign the APK
    let final_apk = config.apk_path();
    sign_apk(toolchain, &aligned_apk, &final_apk, config.profile)?;

    println!("  APK ready: {}", final_apk.display());
    Ok(final_apk)
}

/// Run aapt2 link to create a base APK from the manifest.
fn run_aapt2_link(
    toolchain: &AndroidToolchain,
    manifest_path: &Path,
    output_apk: &Path,
) -> Result<(), BuildError> {
    // Find android.jar for compile SDK
    let platforms_dir = toolchain.sdk_path.join("platforms");
    let android_jar = find_android_jar(&platforms_dir)?;

    let output = Command::new(&toolchain.aapt2_path)
        .arg("link")
        .arg("--manifest")
        .arg(manifest_path)
        .arg("-I")
        .arg(&android_jar)
        .arg("-o")
        .arg(output_apk)
        .arg("--auto-add-overlay")
        .output()
        .map_err(|e| BuildError::ApkPackagingFailed {
            step: "aapt2 link".to_string(),
            stderr: format!("failed to run aapt2: {e}"),
        })?;

    if !output.status.success() {
        return Err(BuildError::ApkPackagingFailed {
            step: "aapt2 link".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// Add the native .so library into the APK using zip.
fn add_native_lib_to_apk(
    apk_path: &Path,
    so_path: &Path,
    build_dir: &Path,
) -> Result<(), BuildError> {
    // We need to add lib/arm64-v8a/libmain.so to the APK
    // Use the `zip` command (available on all platforms) to add the .so
    let lib_relative = PathBuf::from("lib")
        .join(crate::config::QUEST_ABI)
        .join("libmain.so");

    // Ensure the .so is in the right place relative to build_dir
    let so_dest = build_dir.join(&lib_relative);
    if so_path != so_dest {
        if let Some(parent) = so_dest.parent() {
            fs::create_dir_all(parent).map_err(|e| BuildError::IoError {
                context: "creating lib directory in build".to_string(),
                source: e,
            })?;
        }
        fs::copy(so_path, &so_dest).map_err(|e| BuildError::IoError {
            context: "copying .so to build directory".to_string(),
            source: e,
        })?;
    }

    // Use zip to add the library to the APK
    let output = Command::new("zip")
        .arg("-j0") // store without compression (required for native libs)
        .arg(apk_path)
        .arg(&so_dest)
        .current_dir(build_dir)
        .output();

    match output {
        Ok(result) if result.status.success() => return Ok(()),
        _ => {}
    }

    // Fallback: use aapt (part of build-tools) to add the file
    // Re-package by adding the lib directory
    let output = Command::new("zip")
        .args(["-r0", "-g"])
        .arg(apk_path)
        .arg(lib_relative.to_string_lossy().as_ref())
        .current_dir(build_dir)
        .output()
        .map_err(|e| BuildError::ApkPackagingFailed {
            step: "add native library".to_string(),
            stderr: format!("failed to run zip: {e}. Ensure 'zip' is installed."),
        })?;

    if !output.status.success() {
        return Err(BuildError::ApkPackagingFailed {
            step: "add native library".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// Run zipalign on the APK.
fn run_zipalign(
    toolchain: &AndroidToolchain,
    input_apk: &Path,
    output_apk: &Path,
) -> Result<(), BuildError> {
    let output = Command::new(&toolchain.zipalign_path)
        .args(["-f", "-p", "4"])
        .arg(input_apk)
        .arg(output_apk)
        .output()
        .map_err(|e| BuildError::ApkPackagingFailed {
            step: "zipalign".to_string(),
            stderr: format!("failed to run zipalign: {e}"),
        })?;

    if !output.status.success() {
        return Err(BuildError::ApkPackagingFailed {
            step: "zipalign".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// Sign the APK. Uses debug keystore for debug builds.
fn sign_apk(
    toolchain: &AndroidToolchain,
    input_apk: &Path,
    output_apk: &Path,
    profile: BuildProfile,
) -> Result<(), BuildError> {
    // Copy aligned APK to final location first
    fs::copy(input_apk, output_apk).map_err(|e| BuildError::IoError {
        context: "copying aligned APK".to_string(),
        source: e,
    })?;

    let mut cmd = Command::new(&toolchain.apksigner_path);
    cmd.arg("sign");

    match profile {
        BuildProfile::Debug => {
            let debug_keystore = debug_keystore_path();
            if !debug_keystore.exists() {
                println!("  Debug keystore not found, creating...");
                create_debug_keystore(&debug_keystore)?;
            }
            cmd.args(["--ks", &debug_keystore.to_string_lossy()]);
            cmd.args(["--ks-pass", "pass:android"]);
            cmd.args(["--ks-key-alias", "androiddebugkey"]);
        }
        BuildProfile::Release => {
            if let Some((ks_path, ks_pass)) = crate::config::keystore_config() {
                cmd.args(["--ks", &ks_path.to_string_lossy()]);
                cmd.args(["--ks-pass", &format!("pass:{ks_pass}")]);
            } else {
                return Err(BuildError::ApkPackagingFailed {
                    step: "signing".to_string(),
                    stderr: "Release builds require AETHER_KEYSTORE_PATH and AETHER_KEYSTORE_PASSWORD environment variables".to_string(),
                });
            }
        }
    }

    cmd.arg(output_apk);

    let output = cmd.output().map_err(|e| BuildError::ApkPackagingFailed {
        step: "apksigner".to_string(),
        stderr: format!("failed to run apksigner: {e}"),
    })?;

    if !output.status.success() {
        return Err(BuildError::ApkPackagingFailed {
            step: "apksigner".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// Path to the Android debug keystore.
fn debug_keystore_path() -> PathBuf {
    dirs_or_default().join(".android").join("debug.keystore")
}

fn dirs_or_default() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Create a debug keystore using keytool.
fn create_debug_keystore(path: &Path) -> Result<(), BuildError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| BuildError::IoError {
            context: "creating .android directory".to_string(),
            source: e,
        })?;
    }

    let output = Command::new("keytool")
        .args(["-genkeypair"])
        .args(["-keystore", &path.to_string_lossy()])
        .args(["-storepass", "android"])
        .args(["-alias", "androiddebugkey"])
        .args(["-keypass", "android"])
        .args(["-keyalg", "RSA"])
        .args(["-keysize", "2048"])
        .args(["-validity", "10000"])
        .args(["-dname", "CN=Android Debug,O=Android,C=US"])
        .output()
        .map_err(|e| BuildError::ApkPackagingFailed {
            step: "create debug keystore".to_string(),
            stderr: format!("failed to run keytool: {e}. Ensure JDK is installed."),
        })?;

    if !output.status.success() {
        return Err(BuildError::ApkPackagingFailed {
            step: "create debug keystore".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// Find android.jar in the SDK platforms directory.
fn find_android_jar(platforms_dir: &Path) -> Result<PathBuf, BuildError> {
    if !platforms_dir.is_dir() {
        return Err(BuildError::ToolchainNotFound {
            tool: "Android platforms".to_string(),
            hint: format!(
                "No platforms directory at {}. Install a platform via: sdkmanager \"platforms;android-32\"",
                platforms_dir.display()
            ),
        });
    }

    let mut versions: Vec<_> = fs::read_dir(platforms_dir)
        .map_err(|e| BuildError::IoError {
            context: "reading platforms directory".to_string(),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    versions.sort();

    for version in versions.iter().rev() {
        let jar = platforms_dir.join(version).join("android.jar");
        if jar.exists() {
            return Ok(jar);
        }
    }

    Err(BuildError::ToolchainNotFound {
        tool: "android.jar".to_string(),
        hint: "No android.jar found in SDK platforms. Install via: sdkmanager \"platforms;android-32\"".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn build_dir_structure() {
        let tmp = TempDir::new().unwrap();
        let config = BuildConfig::new(
            tmp.path().to_path_buf(),
            crate::config::BuildTarget::Quest,
            BuildProfile::Debug,
        );
        let build_dir = config.build_dir();
        assert!(build_dir
            .to_string_lossy()
            .contains("target/aether-build/quest/build"));
    }

    #[test]
    fn so_placement_path() {
        let lib_path = PathBuf::from("lib")
            .join(crate::config::QUEST_ABI)
            .join("libmain.so");
        assert_eq!(lib_path.to_string_lossy(), "lib/arm64-v8a/libmain.so");
    }

    #[test]
    fn apk_naming_debug() {
        let mut config = BuildConfig::new(
            PathBuf::from("/project"),
            crate::config::BuildTarget::Quest,
            BuildProfile::Debug,
        );
        config.app_name = "myapp".to_string();
        let apk = config.apk_path();
        assert!(apk.to_string_lossy().ends_with("myapp-debug.apk"));
    }

    #[test]
    fn apk_naming_release() {
        let mut config = BuildConfig::new(
            PathBuf::from("/project"),
            crate::config::BuildTarget::Quest,
            BuildProfile::Release,
        );
        config.app_name = "myapp".to_string();
        let apk = config.apk_path();
        assert!(apk.to_string_lossy().ends_with("myapp-release.apk"));
    }

    #[test]
    fn find_android_jar_picks_highest() {
        let tmp = TempDir::new().unwrap();
        let p29 = tmp.path().join("android-29");
        let p32 = tmp.path().join("android-32");
        fs::create_dir_all(&p29).unwrap();
        fs::create_dir_all(&p32).unwrap();
        fs::write(p29.join("android.jar"), "fake").unwrap();
        fs::write(p32.join("android.jar"), "fake").unwrap();

        let result = find_android_jar(tmp.path()).unwrap();
        assert!(result.to_string_lossy().contains("android-32"));
    }

    #[test]
    fn find_android_jar_missing_dir() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("nonexistent");
        let err = find_android_jar(&missing).unwrap_err();
        match err {
            BuildError::ToolchainNotFound { tool, .. } => {
                assert!(tool.contains("platforms"));
            }
            _ => panic!("expected ToolchainNotFound"),
        }
    }

    #[test]
    fn find_android_jar_no_jars() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("android-32")).unwrap();
        // No android.jar inside
        let err = find_android_jar(tmp.path()).unwrap_err();
        match err {
            BuildError::ToolchainNotFound { tool, .. } => {
                assert!(tool.contains("android.jar"));
            }
            _ => panic!("expected ToolchainNotFound"),
        }
    }

    #[test]
    fn debug_keystore_path_in_home() {
        let path = debug_keystore_path();
        assert!(path.to_string_lossy().contains("debug.keystore"));
        assert!(path.to_string_lossy().contains(".android"));
    }

    #[test]
    fn unsigned_apk_name_constant() {
        assert_eq!(UNSIGNED_APK_NAME, "app-unsigned.apk");
    }

    #[test]
    fn aligned_apk_name_constant() {
        assert_eq!(ALIGNED_APK_NAME, "app-aligned.apk");
    }
}
