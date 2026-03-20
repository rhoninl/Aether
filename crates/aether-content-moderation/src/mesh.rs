/// Mesh geometry scanning rules for content moderation.
use crate::scanner::{
    ContentFlag, ContentItem, ContentScanner, ContentType, FlagCategory, ScanResult,
};
use crate::severity::ContentSeverity;

/// Rules for geometry-based content scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryRule {
    /// Mesh resembles a weapon.
    WeaponLike,
    /// Mesh has extreme aspect ratios (potentially offensive shapes).
    ExtremeAspectRatio,
    /// Mesh uses disallowed topology patterns.
    DisallowedTopology,
}

/// A finding from mesh geometry scanning.
#[derive(Debug, Clone)]
pub struct MeshFinding {
    /// Content that was scanned.
    pub content_id: String,
    /// Confidence score 0.0 to 1.0.
    pub score: f32,
    /// Rule that was triggered.
    pub rule: GeometryRule,
}

/// Configurable mesh scanner that checks geometry against rules.
pub struct MeshScanner {
    /// Whether scanning is enabled.
    pub enabled: bool,
    /// Minimum triangle count to trigger scanning (skip tiny meshes).
    pub min_triangles: u32,
    /// Rules to check.
    rules: Vec<GeometryRule>,
}

impl MeshScanner {
    pub fn new(enabled: bool, min_triangles: u32) -> Self {
        Self {
            enabled,
            min_triangles,
            rules: vec![
                GeometryRule::WeaponLike,
                GeometryRule::ExtremeAspectRatio,
                GeometryRule::DisallowedTopology,
            ],
        }
    }

    /// Add a specific rule to check.
    pub fn add_rule(&mut self, rule: GeometryRule) {
        if !self.rules.contains(&rule) {
            self.rules.push(rule);
        }
    }

    /// Check a mesh against all configured rules.
    /// Returns findings with scores based on heuristic analysis.
    pub fn check_mesh(&self, content_id: &str, mesh_data: &[u8]) -> Vec<MeshFinding> {
        if !self.enabled {
            return Vec::new();
        }

        // Skip meshes below minimum triangle count
        // Rough estimate: each triangle needs at least 36 bytes (3 vertices * 3 floats * 4 bytes)
        let estimated_triangles = mesh_data.len() as u32 / 36;
        if estimated_triangles < self.min_triangles {
            return Vec::new();
        }

        let mut findings = Vec::new();

        for rule in &self.rules {
            let score = heuristic_score(rule, mesh_data);
            if score > 0.0 {
                findings.push(MeshFinding {
                    content_id: content_id.to_string(),
                    score,
                    rule: rule.clone(),
                });
            }
        }

        findings
    }
}

impl ContentScanner for MeshScanner {
    fn scan(&self, content: &ContentItem) -> ScanResult {
        if content.content_type != ContentType::Mesh {
            return ScanResult {
                severity: ContentSeverity::Clean,
                flags: Vec::new(),
                confidence: 1.0,
                auto_decision: None,
                scanner_name: self.scanner_name().to_string(),
            };
        }

        let findings = self.check_mesh(&content.content_id, &content.data);

        if findings.is_empty() {
            return ScanResult {
                severity: ContentSeverity::Clean,
                flags: Vec::new(),
                confidence: 0.8,
                auto_decision: None,
                scanner_name: self.scanner_name().to_string(),
            };
        }

        let max_score = findings.iter().map(|f| f.score).fold(0.0_f32, f32::max);

        let severity = if max_score >= 0.9 {
            ContentSeverity::High
        } else if max_score >= 0.6 {
            ContentSeverity::Medium
        } else {
            ContentSeverity::Low
        };

        let flags = findings
            .iter()
            .map(|f| ContentFlag {
                label: format!("{:?} (score: {:.2})", f.rule, f.score),
                category: FlagCategory::Violence,
            })
            .collect();

        ScanResult {
            severity,
            flags,
            confidence: max_score,
            auto_decision: None,
            scanner_name: self.scanner_name().to_string(),
        }
    }

    fn scanner_name(&self) -> &str {
        "mesh_scanner"
    }
}

/// Simple heuristic scoring for mesh data against a rule.
/// In production, this would use ML models or detailed geometry analysis.
fn heuristic_score(rule: &GeometryRule, data: &[u8]) -> f32 {
    match rule {
        GeometryRule::WeaponLike => {
            // Heuristic: check for elongated shapes by looking at data distribution
            if data.len() > 1000 {
                let high_bytes = data.iter().filter(|&&b| b > 200).count();
                let ratio = high_bytes as f32 / data.len() as f32;
                if ratio > 0.3 {
                    return ratio;
                }
            }
            0.0
        }
        GeometryRule::ExtremeAspectRatio => {
            // Heuristic: detect uniform patterns suggesting extreme stretching
            if data.len() > 100 {
                let first = data[0];
                let repeated = data.iter().filter(|&&b| b == first).count();
                let ratio = repeated as f32 / data.len() as f32;
                if ratio > 0.8 {
                    return ratio;
                }
            }
            0.0
        }
        GeometryRule::DisallowedTopology => {
            // Heuristic: detect zero bytes (degenerate triangles)
            if data.len() > 100 {
                let zeros = data.iter().filter(|&&b| b == 0).count();
                let ratio = zeros as f32 / data.len() as f32;
                if ratio > 0.5 {
                    return ratio;
                }
            }
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_scanner_returns_empty() {
        let scanner = MeshScanner::new(false, 0);
        let findings = scanner.check_mesh("mesh-1", &[0u8; 1000]);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_small_mesh_skipped() {
        let scanner = MeshScanner::new(true, 100);
        // 36 bytes = ~1 triangle, below 100 minimum
        let findings = scanner.check_mesh("mesh-1", &[0u8; 36]);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_clean_mesh_no_findings() {
        let scanner = MeshScanner::new(true, 0);
        // Random-ish data that shouldn't trigger heuristics
        let data: Vec<u8> = (0..500).map(|i| ((i * 7 + 13) % 200) as u8).collect();
        let findings = scanner.check_mesh("mesh-1", &data);
        // Should have no or very low findings
        for finding in &findings {
            assert!(finding.score < 0.3);
        }
    }

    #[test]
    fn test_extreme_aspect_ratio_detection() {
        let scanner = MeshScanner::new(true, 0);
        // Highly uniform data triggers extreme aspect ratio
        let data = vec![42u8; 500];
        let findings = scanner.check_mesh("mesh-1", &data);
        let aspect_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.rule == GeometryRule::ExtremeAspectRatio)
            .collect();
        assert!(!aspect_findings.is_empty());
        assert!(aspect_findings[0].score > 0.8);
    }

    #[test]
    fn test_disallowed_topology_detection() {
        let scanner = MeshScanner::new(true, 0);
        // Lots of zero bytes = degenerate triangles
        let data = vec![0u8; 500];
        let findings = scanner.check_mesh("mesh-1", &data);
        let topo_findings: Vec<_> = findings
            .iter()
            .filter(|f| f.rule == GeometryRule::DisallowedTopology)
            .collect();
        assert!(!topo_findings.is_empty());
    }

    #[test]
    fn test_add_rule() {
        let mut scanner = MeshScanner::new(true, 0);
        // Adding a duplicate should not increase count
        let initial_count = scanner.rules.len();
        scanner.add_rule(GeometryRule::WeaponLike);
        assert_eq!(scanner.rules.len(), initial_count);
    }

    #[test]
    fn test_content_scanner_trait_wrong_type() {
        let scanner = MeshScanner::new(true, 0);
        let content = ContentItem {
            content_id: "test".to_string(),
            content_type: ContentType::Audio, // Not Mesh
            data: vec![0u8; 500],
            metadata: std::collections::HashMap::new(),
        };
        let result = scanner.scan(&content);
        assert_eq!(result.severity, ContentSeverity::Clean);
    }

    #[test]
    fn test_content_scanner_trait_clean_mesh() {
        let scanner = MeshScanner::new(true, 0);
        let data: Vec<u8> = (0..500).map(|i| ((i * 7 + 13) % 200) as u8).collect();
        let content = ContentItem {
            content_id: "mesh-1".to_string(),
            content_type: ContentType::Mesh,
            data,
            metadata: std::collections::HashMap::new(),
        };
        let result = scanner.scan(&content);
        assert_eq!(result.scanner_name, "mesh_scanner");
    }

    #[test]
    fn test_content_scanner_trait_flagged_mesh() {
        let scanner = MeshScanner::new(true, 0);
        let content = ContentItem {
            content_id: "mesh-1".to_string(),
            content_type: ContentType::Mesh,
            data: vec![0u8; 500], // triggers degenerate topology
            metadata: std::collections::HashMap::new(),
        };
        let result = scanner.scan(&content);
        assert!(result.severity >= ContentSeverity::Low);
        assert!(!result.flags.is_empty());
    }
}
