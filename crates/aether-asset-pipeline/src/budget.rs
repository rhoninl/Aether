//! Asset budget validation to enforce size and complexity limits.

use std::env;
use std::fmt;

const DEFAULT_MAX_POLYGONS: u32 = 500_000;
const DEFAULT_MAX_MATERIALS: u32 = 32;
const DEFAULT_MAX_TEXTURE_MEMORY_BYTES: u64 = 256 * 1024 * 1024; // 256 MB
const DEFAULT_MAX_TOTAL_SIZE_BYTES: u64 = 512 * 1024 * 1024; // 512 MB

const ENV_MAX_POLYGONS: &str = "AETHER_MAX_POLYGONS";
const ENV_MAX_MATERIALS: &str = "AETHER_MAX_MATERIALS";
const ENV_MAX_TEXTURE_MEMORY: &str = "AETHER_MAX_TEXTURE_MEMORY";
const ENV_MAX_TOTAL_SIZE: &str = "AETHER_MAX_TOTAL_SIZE";

/// Configurable asset budget limits.
#[derive(Debug, Clone)]
pub struct AssetBudget {
    pub max_polygons: u32,
    pub max_materials: u32,
    pub max_texture_memory_bytes: u64,
    pub max_total_size_bytes: u64,
}

impl Default for AssetBudget {
    fn default() -> Self {
        Self {
            max_polygons: DEFAULT_MAX_POLYGONS,
            max_materials: DEFAULT_MAX_MATERIALS,
            max_texture_memory_bytes: DEFAULT_MAX_TEXTURE_MEMORY_BYTES,
            max_total_size_bytes: DEFAULT_MAX_TOTAL_SIZE_BYTES,
        }
    }
}

impl AssetBudget {
    /// Create a budget from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            max_polygons: env::var(ENV_MAX_POLYGONS)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_POLYGONS),
            max_materials: env::var(ENV_MAX_MATERIALS)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_MATERIALS),
            max_texture_memory_bytes: env::var(ENV_MAX_TEXTURE_MEMORY)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_TEXTURE_MEMORY_BYTES),
            max_total_size_bytes: env::var(ENV_MAX_TOTAL_SIZE)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_MAX_TOTAL_SIZE_BYTES),
        }
    }

    /// Create a custom budget with specific limits.
    pub fn new(
        max_polygons: u32,
        max_materials: u32,
        max_texture_memory_bytes: u64,
        max_total_size_bytes: u64,
    ) -> Self {
        Self {
            max_polygons,
            max_materials,
            max_texture_memory_bytes,
            max_total_size_bytes,
        }
    }

    /// Validate asset metrics against this budget.
    pub fn validate(&self, usage: &AssetUsage) -> BudgetReport {
        let mut violations = Vec::new();

        if usage.polygon_count > self.max_polygons {
            violations.push(BudgetViolation {
                category: BudgetCategory::Polygons,
                limit: self.max_polygons as u64,
                actual: usage.polygon_count as u64,
            });
        }

        if usage.material_count > self.max_materials {
            violations.push(BudgetViolation {
                category: BudgetCategory::Materials,
                limit: self.max_materials as u64,
                actual: usage.material_count as u64,
            });
        }

        if usage.texture_memory_bytes > self.max_texture_memory_bytes {
            violations.push(BudgetViolation {
                category: BudgetCategory::TextureMemory,
                limit: self.max_texture_memory_bytes,
                actual: usage.texture_memory_bytes,
            });
        }

        if usage.total_size_bytes > self.max_total_size_bytes {
            violations.push(BudgetViolation {
                category: BudgetCategory::TotalSize,
                limit: self.max_total_size_bytes,
                actual: usage.total_size_bytes,
            });
        }

        BudgetReport {
            passed: violations.is_empty(),
            violations,
            usage: usage.clone(),
        }
    }
}

/// Measured asset usage metrics.
#[derive(Debug, Clone)]
pub struct AssetUsage {
    pub polygon_count: u32,
    pub material_count: u32,
    pub texture_memory_bytes: u64,
    pub total_size_bytes: u64,
}

/// Result of a budget validation check.
#[derive(Debug, Clone)]
pub struct BudgetReport {
    pub passed: bool,
    pub violations: Vec<BudgetViolation>,
    pub usage: AssetUsage,
}

/// Describes a single budget violation.
#[derive(Debug, Clone)]
pub struct BudgetViolation {
    pub category: BudgetCategory,
    pub limit: u64,
    pub actual: u64,
}

impl fmt::Display for BudgetViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} exceeded: limit={}, actual={}",
            self.category, self.limit, self.actual
        )
    }
}

/// Categories of budget limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetCategory {
    Polygons,
    Materials,
    TextureMemory,
    TotalSize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn within_budget_usage() -> AssetUsage {
        AssetUsage {
            polygon_count: 10_000,
            material_count: 5,
            texture_memory_bytes: 1024 * 1024, // 1 MB
            total_size_bytes: 2 * 1024 * 1024,  // 2 MB
        }
    }

    #[test]
    fn default_budget_values() {
        let budget = AssetBudget::default();
        assert_eq!(budget.max_polygons, 500_000);
        assert_eq!(budget.max_materials, 32);
        assert_eq!(budget.max_texture_memory_bytes, 256 * 1024 * 1024);
        assert_eq!(budget.max_total_size_bytes, 512 * 1024 * 1024);
    }

    #[test]
    fn custom_budget_creation() {
        let budget = AssetBudget::new(1000, 4, 1024, 2048);
        assert_eq!(budget.max_polygons, 1000);
        assert_eq!(budget.max_materials, 4);
        assert_eq!(budget.max_texture_memory_bytes, 1024);
        assert_eq!(budget.max_total_size_bytes, 2048);
    }

    #[test]
    fn validate_within_budget_passes() {
        let budget = AssetBudget::default();
        let usage = within_budget_usage();
        let report = budget.validate(&usage);
        assert!(report.passed);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn validate_exceeding_polygons() {
        let budget = AssetBudget::new(100, 32, u64::MAX, u64::MAX);
        let usage = AssetUsage {
            polygon_count: 200,
            material_count: 1,
            texture_memory_bytes: 0,
            total_size_bytes: 0,
        };
        let report = budget.validate(&usage);
        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].category, BudgetCategory::Polygons);
        assert_eq!(report.violations[0].limit, 100);
        assert_eq!(report.violations[0].actual, 200);
    }

    #[test]
    fn validate_exceeding_materials() {
        let budget = AssetBudget::new(u32::MAX, 2, u64::MAX, u64::MAX);
        let usage = AssetUsage {
            polygon_count: 10,
            material_count: 5,
            texture_memory_bytes: 0,
            total_size_bytes: 0,
        };
        let report = budget.validate(&usage);
        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].category, BudgetCategory::Materials);
    }

    #[test]
    fn validate_exceeding_texture_memory() {
        let budget = AssetBudget::new(u32::MAX, u32::MAX, 1024, u64::MAX);
        let usage = AssetUsage {
            polygon_count: 0,
            material_count: 0,
            texture_memory_bytes: 2048,
            total_size_bytes: 0,
        };
        let report = budget.validate(&usage);
        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].category, BudgetCategory::TextureMemory);
    }

    #[test]
    fn validate_exceeding_total_size() {
        let budget = AssetBudget::new(u32::MAX, u32::MAX, u64::MAX, 500);
        let usage = AssetUsage {
            polygon_count: 0,
            material_count: 0,
            texture_memory_bytes: 0,
            total_size_bytes: 1000,
        };
        let report = budget.validate(&usage);
        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].category, BudgetCategory::TotalSize);
    }

    #[test]
    fn validate_multiple_violations() {
        let budget = AssetBudget::new(100, 2, 1024, 2048);
        let usage = AssetUsage {
            polygon_count: 200,
            material_count: 5,
            texture_memory_bytes: 4096,
            total_size_bytes: 8192,
        };
        let report = budget.validate(&usage);
        assert!(!report.passed);
        assert_eq!(report.violations.len(), 4);
    }

    #[test]
    fn validate_at_exact_limit_passes() {
        let budget = AssetBudget::new(100, 5, 1024, 2048);
        let usage = AssetUsage {
            polygon_count: 100,
            material_count: 5,
            texture_memory_bytes: 1024,
            total_size_bytes: 2048,
        };
        let report = budget.validate(&usage);
        assert!(report.passed);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn validate_just_over_limit_fails() {
        let budget = AssetBudget::new(100, 5, 1024, 2048);
        let usage = AssetUsage {
            polygon_count: 101,
            material_count: 5,
            texture_memory_bytes: 1024,
            total_size_bytes: 2048,
        };
        let report = budget.validate(&usage);
        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
    }

    #[test]
    fn validate_zero_usage_passes() {
        let budget = AssetBudget::default();
        let usage = AssetUsage {
            polygon_count: 0,
            material_count: 0,
            texture_memory_bytes: 0,
            total_size_bytes: 0,
        };
        let report = budget.validate(&usage);
        assert!(report.passed);
    }

    #[test]
    fn budget_violation_display() {
        let violation = BudgetViolation {
            category: BudgetCategory::Polygons,
            limit: 100,
            actual: 200,
        };
        let display = format!("{}", violation);
        assert!(display.contains("Polygons"));
        assert!(display.contains("100"));
        assert!(display.contains("200"));
    }

    #[test]
    fn report_contains_usage_data() {
        let budget = AssetBudget::default();
        let usage = within_budget_usage();
        let report = budget.validate(&usage);
        assert_eq!(report.usage.polygon_count, 10_000);
        assert_eq!(report.usage.material_count, 5);
    }
}
