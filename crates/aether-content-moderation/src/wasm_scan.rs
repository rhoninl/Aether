/// WASM bytecode static analysis for detecting malicious patterns.
use crate::severity::ContentSeverity;

/// Categories of WASM violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmViolationKind {
    /// Attempts to import banned host functions.
    BannedImport,
    /// Attempts to access the network.
    NetworkAccess,
    /// Attempts to access the file system.
    FileSystemAccess,
    /// Potential infinite loop pattern.
    InfiniteLoop,
    /// Excessive memory allocation.
    ExcessiveMemory,
    /// Use of deprecated or unsafe instructions.
    UnsafeInstruction,
}

/// A detected violation in WASM bytecode.
#[derive(Debug, Clone)]
pub struct WasmViolation {
    /// Type of violation.
    pub kind: WasmViolationKind,
    /// Byte offset in the WASM binary where the violation was detected.
    pub offset: usize,
    /// Human-readable description.
    pub description: String,
    /// Severity of this violation.
    pub severity: ContentSeverity,
}

/// A pattern to scan for in WASM bytecode.
#[derive(Debug, Clone)]
pub struct WasmPattern {
    /// Name of the pattern.
    pub name: String,
    /// Byte sequence to search for.
    pub pattern_bytes: Vec<u8>,
    /// Violation kind if this pattern matches.
    pub violation_kind: WasmViolationKind,
    /// Severity if matched.
    pub severity: ContentSeverity,
}

/// Result of WASM analysis.
#[derive(Debug)]
pub struct WasmAnalysisResult {
    /// List of violations found.
    pub violations: Vec<WasmViolation>,
    /// Overall severity (maximum of all violations).
    pub overall_severity: ContentSeverity,
    /// Whether the WASM is considered safe.
    pub is_safe: bool,
}

/// Static analyzer for WASM bytecode.
pub struct WasmAnalyzer {
    patterns: Vec<WasmPattern>,
    /// If true, any violation results in rejection.
    strict_mode: bool,
}

/// WASM magic number prefix.
const WASM_MAGIC: &[u8] = b"\x00asm";

/// Maximum allowed memory pages (64KB each). 256 pages = 16MB.
const MAX_MEMORY_PAGES: u32 = 256;

impl WasmAnalyzer {
    pub fn new(strict_mode: bool) -> Self {
        Self {
            patterns: Vec::new(),
            strict_mode,
        }
    }

    /// Create an analyzer with default security patterns.
    pub fn with_default_patterns(strict_mode: bool) -> Self {
        let patterns = vec![
            WasmPattern {
                name: "fd_read syscall".to_string(),
                pattern_bytes: b"fd_read".to_vec(),
                violation_kind: WasmViolationKind::FileSystemAccess,
                severity: ContentSeverity::High,
            },
            WasmPattern {
                name: "fd_write syscall".to_string(),
                pattern_bytes: b"fd_write".to_vec(),
                violation_kind: WasmViolationKind::FileSystemAccess,
                severity: ContentSeverity::Medium,
            },
            WasmPattern {
                name: "sock_send network".to_string(),
                pattern_bytes: b"sock_send".to_vec(),
                violation_kind: WasmViolationKind::NetworkAccess,
                severity: ContentSeverity::Critical,
            },
            WasmPattern {
                name: "sock_recv network".to_string(),
                pattern_bytes: b"sock_recv".to_vec(),
                violation_kind: WasmViolationKind::NetworkAccess,
                severity: ContentSeverity::Critical,
            },
            WasmPattern {
                name: "proc_exit banned".to_string(),
                pattern_bytes: b"proc_exit".to_vec(),
                violation_kind: WasmViolationKind::BannedImport,
                severity: ContentSeverity::High,
            },
        ];

        Self {
            patterns,
            strict_mode,
        }
    }

    /// Add a custom pattern to scan for.
    pub fn add_pattern(&mut self, pattern: WasmPattern) {
        self.patterns.push(pattern);
    }

    /// Analyze WASM bytecode and return results.
    pub fn analyze(&self, wasm_bytes: &[u8]) -> WasmAnalysisResult {
        let mut violations = Vec::new();

        // Check WASM magic number
        if wasm_bytes.len() < 4 || &wasm_bytes[0..4] != WASM_MAGIC {
            violations.push(WasmViolation {
                kind: WasmViolationKind::UnsafeInstruction,
                offset: 0,
                description: "invalid WASM magic number".to_string(),
                severity: ContentSeverity::Critical,
            });
        }

        // Scan for pattern matches
        for pattern in &self.patterns {
            for offset in find_all_occurrences(wasm_bytes, &pattern.pattern_bytes) {
                violations.push(WasmViolation {
                    kind: pattern.violation_kind.clone(),
                    offset,
                    description: format!("pattern match: {}", pattern.name),
                    severity: pattern.severity,
                });
            }
        }

        // Check for excessive memory declarations
        // WASM memory section uses 0x05 as section ID
        if let Some(violation) = check_memory_limits(wasm_bytes) {
            violations.push(violation);
        }

        let overall_severity = violations
            .iter()
            .map(|v| v.severity)
            .max()
            .unwrap_or(ContentSeverity::Clean);

        let is_safe = if self.strict_mode {
            violations.is_empty()
        } else {
            overall_severity < ContentSeverity::High
        };

        WasmAnalysisResult {
            violations,
            overall_severity,
            is_safe,
        }
    }

    /// Returns whether strict mode is enabled.
    pub fn is_strict(&self) -> bool {
        self.strict_mode
    }
}

/// Find all byte offsets where `needle` occurs in `haystack`.
fn find_all_occurrences(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }

    let mut positions = Vec::new();
    for i in 0..=(haystack.len() - needle.len()) {
        if haystack[i..i + needle.len()] == *needle {
            positions.push(i);
        }
    }
    positions
}

/// Check for memory section with excessive page counts.
/// This is a simplified heuristic: looks for the memory section byte pattern.
fn check_memory_limits(wasm_bytes: &[u8]) -> Option<WasmViolation> {
    // Look for memory section (section ID 0x05)
    // Format: 0x05 <section_size> <num_memories> <limits>
    // limits: 0x00 <min_pages> OR 0x01 <min_pages> <max_pages>
    for i in 0..wasm_bytes.len().saturating_sub(4) {
        if wasm_bytes[i] == 0x05 {
            // Check if there's enough room for a memory declaration
            if i + 4 < wasm_bytes.len() {
                let potential_pages = wasm_bytes[i + 3] as u32;
                if potential_pages > MAX_MEMORY_PAGES as u8 as u32 {
                    return Some(WasmViolation {
                        kind: WasmViolationKind::ExcessiveMemory,
                        offset: i,
                        description: format!(
                            "excessive memory: {} pages (max {})",
                            potential_pages, MAX_MEMORY_PAGES
                        ),
                        severity: ContentSeverity::Medium,
                    });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_valid_wasm() -> Vec<u8> {
        let mut bytes = WASM_MAGIC.to_vec();
        // Version 1
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        bytes
    }

    fn make_wasm_with_import(import_name: &[u8]) -> Vec<u8> {
        let mut bytes = make_valid_wasm();
        bytes.extend_from_slice(import_name);
        bytes
    }

    #[test]
    fn test_valid_wasm_no_patterns() {
        let analyzer = WasmAnalyzer::new(false);
        let wasm = make_valid_wasm();
        let result = analyzer.analyze(&wasm);
        assert!(result.violations.is_empty());
        assert_eq!(result.overall_severity, ContentSeverity::Clean);
        assert!(result.is_safe);
    }

    #[test]
    fn test_invalid_magic_number() {
        let analyzer = WasmAnalyzer::new(false);
        let bytes = vec![0x00, 0x00, 0x00, 0x00];
        let result = analyzer.analyze(&bytes);
        assert!(!result.violations.is_empty());
        assert_eq!(
            result.violations[0].kind,
            WasmViolationKind::UnsafeInstruction
        );
        assert_eq!(result.violations[0].severity, ContentSeverity::Critical);
    }

    #[test]
    fn test_empty_bytes() {
        let analyzer = WasmAnalyzer::new(false);
        let result = analyzer.analyze(&[]);
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn test_pattern_detection_fd_read() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let wasm = make_wasm_with_import(b"fd_read");
        let result = analyzer.analyze(&wasm);

        let fs_violations: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.kind == WasmViolationKind::FileSystemAccess)
            .collect();
        assert!(!fs_violations.is_empty());
    }

    #[test]
    fn test_pattern_detection_network() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let wasm = make_wasm_with_import(b"sock_send");
        let result = analyzer.analyze(&wasm);

        let net_violations: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.kind == WasmViolationKind::NetworkAccess)
            .collect();
        assert!(!net_violations.is_empty());
        assert_eq!(result.overall_severity, ContentSeverity::Critical);
    }

    #[test]
    fn test_pattern_detection_sock_recv() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let wasm = make_wasm_with_import(b"sock_recv");
        let result = analyzer.analyze(&wasm);

        let net_violations: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.kind == WasmViolationKind::NetworkAccess)
            .collect();
        assert!(!net_violations.is_empty());
    }

    #[test]
    fn test_multiple_violations() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let mut wasm = make_valid_wasm();
        wasm.extend_from_slice(b"fd_read");
        wasm.extend_from_slice(b"---");
        wasm.extend_from_slice(b"sock_send");
        let result = analyzer.analyze(&wasm);

        // Should have both filesystem and network violations
        let kinds: Vec<_> = result.violations.iter().map(|v| &v.kind).collect();
        assert!(kinds.contains(&&WasmViolationKind::FileSystemAccess));
        assert!(kinds.contains(&&WasmViolationKind::NetworkAccess));
    }

    #[test]
    fn test_strict_mode_any_violation_unsafe() {
        let analyzer = WasmAnalyzer::with_default_patterns(true);
        let wasm = make_wasm_with_import(b"fd_write");
        let result = analyzer.analyze(&wasm);

        assert!(!result.is_safe); // strict mode: any violation = unsafe
    }

    #[test]
    fn test_non_strict_mode_medium_is_safe() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let wasm = make_wasm_with_import(b"fd_write");
        let result = analyzer.analyze(&wasm);

        // fd_write is Medium severity, which is below High threshold in non-strict mode
        assert!(result.is_safe);
    }

    #[test]
    fn test_non_strict_mode_critical_is_unsafe() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let wasm = make_wasm_with_import(b"sock_send");
        let result = analyzer.analyze(&wasm);

        assert!(!result.is_safe);
    }

    #[test]
    fn test_custom_pattern() {
        let mut analyzer = WasmAnalyzer::new(false);
        analyzer.add_pattern(WasmPattern {
            name: "custom_evil".to_string(),
            pattern_bytes: b"evil_func".to_vec(),
            violation_kind: WasmViolationKind::BannedImport,
            severity: ContentSeverity::High,
        });

        let wasm = make_wasm_with_import(b"evil_func");
        let result = analyzer.analyze(&wasm);

        let banned: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.kind == WasmViolationKind::BannedImport)
            .collect();
        assert!(!banned.is_empty());
    }

    #[test]
    fn test_clean_wasm_with_default_patterns() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let wasm = make_valid_wasm();
        let result = analyzer.analyze(&wasm);

        // No default patterns match in valid WASM with no imports
        assert!(result.is_safe);
        assert_eq!(result.overall_severity, ContentSeverity::Clean);
    }

    #[test]
    fn test_find_all_occurrences_basic() {
        let haystack = b"abcabcabc";
        let needle = b"abc";
        let positions = find_all_occurrences(haystack, needle);
        assert_eq!(positions, vec![0, 3, 6]);
    }

    #[test]
    fn test_find_all_occurrences_no_match() {
        let haystack = b"abcdef";
        let needle = b"xyz";
        let positions = find_all_occurrences(haystack, needle);
        assert!(positions.is_empty());
    }

    #[test]
    fn test_find_all_occurrences_empty_needle() {
        let haystack = b"abc";
        let needle = b"";
        let positions = find_all_occurrences(haystack, needle);
        assert!(positions.is_empty());
    }

    #[test]
    fn test_find_all_occurrences_needle_longer_than_haystack() {
        let haystack = b"ab";
        let needle = b"abcdef";
        let positions = find_all_occurrences(haystack, needle);
        assert!(positions.is_empty());
    }

    #[test]
    fn test_is_strict() {
        let analyzer = WasmAnalyzer::new(true);
        assert!(analyzer.is_strict());

        let analyzer = WasmAnalyzer::new(false);
        assert!(!analyzer.is_strict());
    }

    #[test]
    fn test_proc_exit_detection() {
        let analyzer = WasmAnalyzer::with_default_patterns(false);
        let wasm = make_wasm_with_import(b"proc_exit");
        let result = analyzer.analyze(&wasm);

        let banned: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.kind == WasmViolationKind::BannedImport)
            .collect();
        assert!(!banned.is_empty());
    }

    #[test]
    fn test_violation_offset_correct() {
        let mut analyzer = WasmAnalyzer::new(false);
        analyzer.add_pattern(WasmPattern {
            name: "test".to_string(),
            pattern_bytes: b"EVIL".to_vec(),
            violation_kind: WasmViolationKind::BannedImport,
            severity: ContentSeverity::High,
        });

        let mut wasm = make_valid_wasm(); // 8 bytes
        wasm.extend_from_slice(b"safe_EVIL_end");
        let result = analyzer.analyze(&wasm);

        let banned: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.kind == WasmViolationKind::BannedImport)
            .collect();
        assert_eq!(banned.len(), 1);
        // "EVIL" starts at offset 8 (wasm header) + 5 (safe_) = 13
        assert_eq!(banned[0].offset, 13);
    }
}
