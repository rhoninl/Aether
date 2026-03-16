pub fn print_version() {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    println!("{name} {version}");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_version_env_vars_set() {
        // These are always set by cargo during build
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
        assert_eq!(version, "0.1.0");
    }
}
