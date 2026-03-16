use std::env;
use std::process::Command;

use crate::{AVAILABLE_EXAMPLES, example_binary_name};

pub fn list_examples() {
    println!("Available examples:");
    for example in AVAILABLE_EXAMPLES {
        println!("  {example}");
    }
}

pub fn run_example(name: &str) -> Result<(), String> {
    let binary = example_binary_name(name)
        .ok_or_else(|| format!("unknown example '{name}'. Run 'aether run --list' to see available examples"))?;

    let self_path = env::current_exe().map_err(|e| format!("failed to get executable path: {e}"))?;
    let bin_dir = self_path
        .parent()
        .ok_or_else(|| "failed to determine binary directory".to_string())?;

    let binary_path = bin_dir.join(binary);

    if !binary_path.exists() {
        return Err(format!(
            "binary '{binary}' not found at {}. It may not have been bundled in this distribution.",
            binary_path.display()
        ));
    }

    let status = Command::new(&binary_path)
        .status()
        .map_err(|e| format!("failed to launch '{binary}': {e}"))?;

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        return Err(format!("'{binary}' exited with code {code}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_unknown_example() {
        let result = run_example("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown example"));
    }

    #[test]
    fn test_run_missing_binary() {
        // The binary won't exist in the test build directory
        let result = run_example("3d-demo");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should fail because the sibling binary doesn't exist, not because the name is unknown
        assert!(
            err.contains("not found") || err.contains("failed to launch"),
            "unexpected error: {err}"
        );
    }
}
