pub mod commands;
pub mod manifest;

pub const AVAILABLE_EXAMPLES: &[&str] = &[
    "3d-demo",
    "lua-scripting",
    "vr-emulator",
];

/// Map a user-facing example name to the actual binary name.
pub fn example_binary_name(example: &str) -> Option<&'static str> {
    match example {
        "3d-demo" => Some("aether-3d-demo"),
        "lua-scripting" => Some("aether-lua-demo"),
        "vr-emulator" => Some("aether-vr-emulator-demo"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_examples_not_empty() {
        assert!(!AVAILABLE_EXAMPLES.is_empty());
    }

    #[test]
    fn test_all_examples_have_binary_mapping() {
        for example in AVAILABLE_EXAMPLES {
            assert!(
                example_binary_name(example).is_some(),
                "missing binary mapping for '{example}'"
            );
        }
    }

    #[test]
    fn test_example_binary_names() {
        assert_eq!(example_binary_name("3d-demo"), Some("aether-3d-demo"));
        assert_eq!(example_binary_name("lua-scripting"), Some("aether-lua-demo"));
        assert_eq!(example_binary_name("vr-emulator"), Some("aether-vr-emulator-demo"));
    }

    #[test]
    fn test_unknown_example_returns_none() {
        assert_eq!(example_binary_name("nonexistent"), None);
        assert_eq!(example_binary_name(""), None);
    }
}
