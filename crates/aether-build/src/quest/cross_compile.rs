use std::path::PathBuf;
use std::process::Command;

use crate::config::{BuildConfig, BuildProfile, QUEST_ABI, QUEST_RUST_TARGET};
use crate::toolchain::AndroidToolchain;
use crate::BuildError;

/// Cross-compile the project for Quest (aarch64-linux-android).
/// Returns the path to the compiled shared library.
pub fn compile(config: &BuildConfig, toolchain: &AndroidToolchain) -> Result<PathBuf, BuildError> {
    println!("  Cross-compiling for {QUEST_RUST_TARGET}...");

    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    cmd.args(["--target", QUEST_RUST_TARGET]);

    if let Some(pkg) = &config.package {
        cmd.args(["-p", pkg]);
    } else {
        cmd.arg("--lib");
    }

    if config.profile == BuildProfile::Release {
        cmd.arg("--release");
    }

    cmd.current_dir(&config.project_dir);

    // Set NDK toolchain environment variables
    let clang = toolchain.ndk_clang_path.to_string_lossy().to_string();
    let ar = toolchain.ndk_ar_path.to_string_lossy().to_string();
    let target_env = QUEST_RUST_TARGET.to_uppercase().replace('-', "_");

    cmd.env(format!("CC_{QUEST_RUST_TARGET}"), &clang);
    cmd.env(format!("AR_{QUEST_RUST_TARGET}"), &ar);
    cmd.env(format!("CARGO_TARGET_{target_env}_LINKER"), &clang);

    let output = cmd.output().map_err(|e| BuildError::CompilationFailed {
        stderr: format!("failed to run cargo: {e}"),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BuildError::CompilationFailed {
            stderr: stderr.to_string(),
        });
    }

    let so_path = find_compiled_so(config)?;

    // Copy .so to build directory
    let dest_dir = config.build_dir().join("lib").join(QUEST_ABI);
    std::fs::create_dir_all(&dest_dir).map_err(|e| BuildError::IoError {
        context: "creating lib output directory".to_string(),
        source: e,
    })?;

    let dest = dest_dir.join("libmain.so");
    std::fs::copy(&so_path, &dest).map_err(|e| BuildError::IoError {
        context: format!("copying .so to {}", dest.display()),
        source: e,
    })?;

    println!("  Compiled: {}", dest.display());
    Ok(dest)
}

/// Build the cross-compilation environment variables (for testing).
pub fn build_env_vars(toolchain: &AndroidToolchain) -> Vec<(String, String)> {
    let clang = toolchain.ndk_clang_path.to_string_lossy().to_string();
    let ar = toolchain.ndk_ar_path.to_string_lossy().to_string();
    let target_env = QUEST_RUST_TARGET.to_uppercase().replace('-', "_");

    vec![
        (format!("CC_{QUEST_RUST_TARGET}"), clang.clone()),
        (format!("AR_{QUEST_RUST_TARGET}"), ar),
        (format!("CARGO_TARGET_{target_env}_LINKER"), clang),
    ]
}

/// Find the compiled .so file in the cargo target directory.
fn find_compiled_so(config: &BuildConfig) -> Result<PathBuf, BuildError> {
    let profile_dir = match config.profile {
        BuildProfile::Debug => "debug",
        BuildProfile::Release => "release",
    };

    let target_dir = config
        .project_dir
        .join("target")
        .join(QUEST_RUST_TARGET)
        .join(profile_dir);

    // Look for any .so file (lib<name>.so)
    if target_dir.is_dir() {
        let entries = std::fs::read_dir(&target_dir).map_err(|e| BuildError::IoError {
            context: "reading target directory".to_string(),
            source: e,
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "so" {
                    return Ok(path);
                }
            }
        }
    }

    Err(BuildError::CompilationFailed {
        stderr: format!(
            "no .so file found in {}. Ensure the crate has [lib] with crate-type = [\"cdylib\"]",
            target_dir.display()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mock_toolchain() -> AndroidToolchain {
        AndroidToolchain {
            ndk_path: PathBuf::from("/ndk"),
            sdk_path: PathBuf::from("/sdk"),
            aapt2_path: PathBuf::from("/sdk/build-tools/34.0.0/aapt2"),
            zipalign_path: PathBuf::from("/sdk/build-tools/34.0.0/zipalign"),
            apksigner_path: PathBuf::from("/sdk/build-tools/34.0.0/apksigner"),
            ndk_clang_path: PathBuf::from("/ndk/toolchains/llvm/prebuilt/host/bin/clang"),
            ndk_ar_path: PathBuf::from("/ndk/toolchains/llvm/prebuilt/host/bin/llvm-ar"),
        }
    }

    #[test]
    fn env_vars_contain_cc() {
        let tc = mock_toolchain();
        let vars = build_env_vars(&tc);
        assert!(vars.iter().any(|(k, _)| k == "CC_aarch64-linux-android"));
    }

    #[test]
    fn env_vars_contain_ar() {
        let tc = mock_toolchain();
        let vars = build_env_vars(&tc);
        assert!(vars.iter().any(|(k, _)| k == "AR_aarch64-linux-android"));
    }

    #[test]
    fn env_vars_contain_linker() {
        let tc = mock_toolchain();
        let vars = build_env_vars(&tc);
        assert!(vars
            .iter()
            .any(|(k, _)| k == "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"));
    }

    #[test]
    fn env_vars_use_clang_path() {
        let tc = mock_toolchain();
        let vars = build_env_vars(&tc);
        let cc = vars
            .iter()
            .find(|(k, _)| k == "CC_aarch64-linux-android")
            .unwrap();
        assert!(cc.1.contains("clang"));
    }

    #[test]
    fn find_so_in_target_dir() {
        let tmp = TempDir::new().unwrap();
        let so_dir = tmp
            .path()
            .join("target")
            .join(QUEST_RUST_TARGET)
            .join("debug");
        std::fs::create_dir_all(&so_dir).unwrap();
        std::fs::write(so_dir.join("libmyapp.so"), "fake-so").unwrap();

        let config = BuildConfig::new(
            tmp.path().to_path_buf(),
            crate::config::BuildTarget::Quest,
            BuildProfile::Debug,
        );

        let result = find_compiled_so(&config).unwrap();
        assert!(result.to_string_lossy().contains("libmyapp.so"));
    }

    #[test]
    fn find_so_missing() {
        let tmp = TempDir::new().unwrap();
        let config = BuildConfig::new(
            tmp.path().to_path_buf(),
            crate::config::BuildTarget::Quest,
            BuildProfile::Debug,
        );
        let err = find_compiled_so(&config).unwrap_err();
        match err {
            BuildError::CompilationFailed { stderr } => {
                assert!(stderr.contains("cdylib"));
            }
            _ => panic!("expected CompilationFailed"),
        }
    }
}
