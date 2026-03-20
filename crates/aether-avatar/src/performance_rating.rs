//! Performance budget enforcement for avatars.
//!
//! Validates avatar mesh statistics against per-tier polygon, material,
//! and bone count budgets. Enforces world-level minimum performance
//! requirements.

use crate::rating::{AvatarBudget, AvatarRatingBucket};

/// Polygon budget for Tier S avatars.
const TIER_S_MAX_POLYGONS: u32 = 10_000;
/// Polygon budget for Tier A avatars.
const TIER_A_MAX_POLYGONS: u32 = 25_000;
/// Polygon budget for Tier B avatars.
const TIER_B_MAX_POLYGONS: u32 = 50_000;
/// Polygon budget for Tier C avatars.
const TIER_C_MAX_POLYGONS: u32 = 75_000;

/// Material slot limit for Tier S.
const TIER_S_MAX_MATERIALS: u8 = 2;
/// Material slot limit for Tier A.
const TIER_A_MAX_MATERIALS: u8 = 4;
/// Material slot limit for Tier B.
const TIER_B_MAX_MATERIALS: u8 = 8;
/// Material slot limit for Tier C.
const TIER_C_MAX_MATERIALS: u8 = 16;

/// Bone count limit for Tier S.
const TIER_S_MAX_BONES: u16 = 64;
/// Bone count limit for Tier A.
const TIER_A_MAX_BONES: u16 = 128;
/// Bone count limit for Tier B.
const TIER_B_MAX_BONES: u16 = 256;
/// Bone count limit for Tier C.
const TIER_C_MAX_BONES: u16 = 512;

/// Mesh statistics for an avatar, used for performance validation.
#[derive(Debug, Clone)]
pub struct AvatarMeshStats {
    /// Total polygon (triangle) count.
    pub polygon_count: u32,
    /// Number of material slots used.
    pub material_count: u8,
    /// Number of bones in the skeleton.
    pub bone_count: u16,
    /// Number of blend shape targets.
    pub blend_shape_count: u16,
    /// Total texture memory in bytes.
    pub texture_memory_bytes: u64,
}

/// A table of performance budgets per tier.
#[derive(Debug, Clone)]
pub struct PerformanceBudgetTable {
    /// Budget for Tier S.
    pub tier_s: AvatarBudget,
    /// Budget for Tier A.
    pub tier_a: AvatarBudget,
    /// Budget for Tier B.
    pub tier_b: AvatarBudget,
    /// Budget for Tier C.
    pub tier_c: AvatarBudget,
}

impl Default for PerformanceBudgetTable {
    fn default() -> Self {
        Self {
            tier_s: AvatarBudget {
                max_polygons: TIER_S_MAX_POLYGONS,
                max_material_layers: TIER_S_MAX_MATERIALS,
                max_bone_count: TIER_S_MAX_BONES,
            },
            tier_a: AvatarBudget {
                max_polygons: TIER_A_MAX_POLYGONS,
                max_material_layers: TIER_A_MAX_MATERIALS,
                max_bone_count: TIER_A_MAX_BONES,
            },
            tier_b: AvatarBudget {
                max_polygons: TIER_B_MAX_POLYGONS,
                max_material_layers: TIER_B_MAX_MATERIALS,
                max_bone_count: TIER_B_MAX_BONES,
            },
            tier_c: AvatarBudget {
                max_polygons: TIER_C_MAX_POLYGONS,
                max_material_layers: TIER_C_MAX_MATERIALS,
                max_bone_count: TIER_C_MAX_BONES,
            },
        }
    }
}

impl PerformanceBudgetTable {
    /// Get the budget for a specific tier.
    pub fn budget_for_tier(&self, tier: &AvatarRatingBucket) -> &AvatarBudget {
        match tier {
            AvatarRatingBucket::TierS => &self.tier_s,
            AvatarRatingBucket::TierA => &self.tier_a,
            AvatarRatingBucket::TierB => &self.tier_b,
            AvatarRatingBucket::TierC => &self.tier_c,
        }
    }
}

/// Result of validating an avatar against performance budgets.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// The best tier this avatar qualifies for.
    pub achieved_tier: AvatarRatingBucket,
    /// Whether the avatar passes the world's minimum tier requirement.
    pub passes_world_minimum: bool,
    /// Specific violations found.
    pub violations: Vec<BudgetViolation>,
    /// Computed performance score (0.0 = worst, 1.0 = best within tier).
    pub score: f32,
}

/// A specific budget violation.
#[derive(Debug, Clone)]
pub struct BudgetViolation {
    /// Which resource is over budget.
    pub resource: BudgetResource,
    /// The budget limit.
    pub limit: u64,
    /// The actual value.
    pub actual: u64,
    /// The tier this violation applies to.
    pub tier: AvatarRatingBucket,
}

/// Types of resources that can exceed budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetResource {
    Polygons,
    Materials,
    Bones,
}

/// Determine which tier an avatar qualifies for based on mesh stats.
///
/// Returns the best (most restrictive) tier whose budgets are all met.
pub fn classify_avatar(
    stats: &AvatarMeshStats,
    table: &PerformanceBudgetTable,
) -> AvatarRatingBucket {
    if fits_budget(stats, &table.tier_s) {
        AvatarRatingBucket::TierS
    } else if fits_budget(stats, &table.tier_a) {
        AvatarRatingBucket::TierA
    } else if fits_budget(stats, &table.tier_b) {
        AvatarRatingBucket::TierB
    } else {
        AvatarRatingBucket::TierC
    }
}

/// Check whether mesh stats fit within a budget.
fn fits_budget(stats: &AvatarMeshStats, budget: &AvatarBudget) -> bool {
    stats.polygon_count <= budget.max_polygons
        && stats.material_count <= budget.max_material_layers
        && stats.bone_count <= budget.max_bone_count
}

/// Validate an avatar against the budget table and a world minimum tier.
pub fn validate_avatar(
    stats: &AvatarMeshStats,
    table: &PerformanceBudgetTable,
    world_min_tier: &AvatarRatingBucket,
) -> ValidationResult {
    let achieved = classify_avatar(stats, table);
    let violations = collect_violations(stats, table, &achieved);

    let passes = tier_ordinal(&achieved) <= tier_ordinal(world_min_tier);

    let score = compute_score(stats, table.budget_for_tier(&achieved));

    ValidationResult {
        achieved_tier: achieved,
        passes_world_minimum: passes,
        violations,
        score,
    }
}

/// Collect all budget violations for the tier the avatar was classified into.
fn collect_violations(
    stats: &AvatarMeshStats,
    table: &PerformanceBudgetTable,
    tier: &AvatarRatingBucket,
) -> Vec<BudgetViolation> {
    let budget = table.budget_for_tier(tier);
    let mut violations = Vec::new();

    if stats.polygon_count > budget.max_polygons {
        violations.push(BudgetViolation {
            resource: BudgetResource::Polygons,
            limit: budget.max_polygons as u64,
            actual: stats.polygon_count as u64,
            tier: tier.clone(),
        });
    }
    if stats.material_count > budget.max_material_layers {
        violations.push(BudgetViolation {
            resource: BudgetResource::Materials,
            limit: budget.max_material_layers as u64,
            actual: stats.material_count as u64,
            tier: tier.clone(),
        });
    }
    if stats.bone_count > budget.max_bone_count {
        violations.push(BudgetViolation {
            resource: BudgetResource::Bones,
            limit: budget.max_bone_count as u64,
            actual: stats.bone_count as u64,
            tier: tier.clone(),
        });
    }

    violations
}

/// Compute a performance score (0.0-1.0) relative to a budget.
/// Lower resource usage = higher score.
fn compute_score(stats: &AvatarMeshStats, budget: &AvatarBudget) -> f32 {
    let poly_ratio = if budget.max_polygons > 0 {
        1.0 - (stats.polygon_count as f32 / budget.max_polygons as f32).min(1.0)
    } else {
        0.0
    };
    let mat_ratio = if budget.max_material_layers > 0 {
        1.0 - (stats.material_count as f32 / budget.max_material_layers as f32).min(1.0)
    } else {
        0.0
    };
    let bone_ratio = if budget.max_bone_count > 0 {
        1.0 - (stats.bone_count as f32 / budget.max_bone_count as f32).min(1.0)
    } else {
        0.0
    };

    // Weighted average: polygons matter most
    (poly_ratio * 0.6 + mat_ratio * 0.2 + bone_ratio * 0.2).clamp(0.0, 1.0)
}

/// Map tier to ordinal for comparison (lower = stricter/better).
fn tier_ordinal(tier: &AvatarRatingBucket) -> u8 {
    match tier {
        AvatarRatingBucket::TierS => 0,
        AvatarRatingBucket::TierA => 1,
        AvatarRatingBucket::TierB => 2,
        AvatarRatingBucket::TierC => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_table() -> PerformanceBudgetTable {
        PerformanceBudgetTable::default()
    }

    fn make_stats(polygons: u32, materials: u8, bones: u16) -> AvatarMeshStats {
        AvatarMeshStats {
            polygon_count: polygons,
            material_count: materials,
            bone_count: bones,
            blend_shape_count: 0,
            texture_memory_bytes: 0,
        }
    }

    #[test]
    fn test_default_budget_table_values() {
        let table = default_table();
        assert_eq!(table.tier_s.max_polygons, TIER_S_MAX_POLYGONS);
        assert_eq!(table.tier_a.max_polygons, TIER_A_MAX_POLYGONS);
        assert_eq!(table.tier_b.max_polygons, TIER_B_MAX_POLYGONS);
        assert_eq!(table.tier_c.max_polygons, TIER_C_MAX_POLYGONS);
    }

    #[test]
    fn test_classify_tier_s() {
        let stats = make_stats(5_000, 1, 32);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierS));
    }

    #[test]
    fn test_classify_tier_s_at_limits() {
        let stats = make_stats(TIER_S_MAX_POLYGONS, TIER_S_MAX_MATERIALS, TIER_S_MAX_BONES);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierS));
    }

    #[test]
    fn test_classify_tier_a_polygons() {
        let stats = make_stats(15_000, 2, 64);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierA));
    }

    #[test]
    fn test_classify_tier_a_materials() {
        let stats = make_stats(5_000, 3, 32);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierA));
    }

    #[test]
    fn test_classify_tier_a_bones() {
        let stats = make_stats(5_000, 1, 100);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierA));
    }

    #[test]
    fn test_classify_tier_b() {
        let stats = make_stats(40_000, 6, 200);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierB));
    }

    #[test]
    fn test_classify_tier_c_polygons() {
        let stats = make_stats(60_000, 4, 128);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierC));
    }

    #[test]
    fn test_classify_tier_c_all_over() {
        let stats = make_stats(100_000, 20, 600);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierC));
    }

    #[test]
    fn test_classify_zero_stats() {
        let stats = make_stats(0, 0, 0);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierS));
    }

    #[test]
    fn test_validate_passes_world_min_same_tier() {
        let stats = make_stats(5_000, 1, 32);
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierS);
        assert!(result.passes_world_minimum);
    }

    #[test]
    fn test_validate_passes_world_min_better_tier() {
        let stats = make_stats(5_000, 1, 32); // Tier S
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierB);
        assert!(result.passes_world_minimum);
    }

    #[test]
    fn test_validate_fails_world_min_worse_tier() {
        let stats = make_stats(60_000, 10, 300); // Tier C
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierA);
        assert!(!result.passes_world_minimum);
    }

    #[test]
    fn test_validate_no_violations_when_within_budget() {
        let stats = make_stats(5_000, 1, 32);
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierC);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_validate_violations_tier_c_over_budget() {
        let stats = make_stats(100_000, 20, 600);
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierC);
        // Classified as Tier C, but exceeds Tier C budgets
        assert!(!result.violations.is_empty());
        let poly_violation = result
            .violations
            .iter()
            .find(|v| v.resource == BudgetResource::Polygons);
        assert!(poly_violation.is_some());
    }

    #[test]
    fn test_validate_score_perfect() {
        let stats = make_stats(0, 0, 0);
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierC);
        assert!((result.score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_validate_score_at_budget() {
        let stats = make_stats(TIER_S_MAX_POLYGONS, TIER_S_MAX_MATERIALS, TIER_S_MAX_BONES);
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierC);
        assert!(result.score.abs() < 0.01); // At budget = score 0
    }

    #[test]
    fn test_validate_score_half_budget() {
        let stats = make_stats(
            TIER_S_MAX_POLYGONS / 2,
            TIER_S_MAX_MATERIALS / 2,
            TIER_S_MAX_BONES / 2,
        );
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierC);
        assert!(result.score > 0.4);
        assert!(result.score < 0.6);
    }

    #[test]
    fn test_budget_for_tier() {
        let table = default_table();
        assert_eq!(
            table
                .budget_for_tier(&AvatarRatingBucket::TierS)
                .max_polygons,
            TIER_S_MAX_POLYGONS
        );
        assert_eq!(
            table
                .budget_for_tier(&AvatarRatingBucket::TierA)
                .max_polygons,
            TIER_A_MAX_POLYGONS
        );
        assert_eq!(
            table
                .budget_for_tier(&AvatarRatingBucket::TierB)
                .max_polygons,
            TIER_B_MAX_POLYGONS
        );
        assert_eq!(
            table
                .budget_for_tier(&AvatarRatingBucket::TierC)
                .max_polygons,
            TIER_C_MAX_POLYGONS
        );
    }

    #[test]
    fn test_tier_ordinal_ordering() {
        assert!(
            tier_ordinal(&AvatarRatingBucket::TierS) < tier_ordinal(&AvatarRatingBucket::TierA)
        );
        assert!(
            tier_ordinal(&AvatarRatingBucket::TierA) < tier_ordinal(&AvatarRatingBucket::TierB)
        );
        assert!(
            tier_ordinal(&AvatarRatingBucket::TierB) < tier_ordinal(&AvatarRatingBucket::TierC)
        );
    }

    #[test]
    fn test_classify_boundary_single_constraint_over() {
        // Polygons at S limit, but materials just over S limit
        let stats = make_stats(
            TIER_S_MAX_POLYGONS,
            TIER_S_MAX_MATERIALS + 1,
            TIER_S_MAX_BONES,
        );
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierA));
    }

    #[test]
    fn test_budget_violation_detail() {
        let stats = make_stats(100_000, 20, 600);
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierC);
        for v in &result.violations {
            assert!(v.actual > v.limit);
        }
    }

    #[test]
    fn test_validate_tier_b_world_min_tier_b() {
        let stats = make_stats(40_000, 6, 200);
        let result = validate_avatar(&stats, &default_table(), &AvatarRatingBucket::TierB);
        assert!(result.passes_world_minimum);
        assert!(matches!(result.achieved_tier, AvatarRatingBucket::TierB));
    }

    #[test]
    fn test_classify_just_over_tier_s_polygons() {
        let stats = make_stats(TIER_S_MAX_POLYGONS + 1, 1, 32);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierA));
    }

    #[test]
    fn test_classify_just_over_tier_a_polygons() {
        let stats = make_stats(TIER_A_MAX_POLYGONS + 1, 1, 32);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierB));
    }

    #[test]
    fn test_classify_just_over_tier_b_polygons() {
        let stats = make_stats(TIER_B_MAX_POLYGONS + 1, 1, 32);
        let tier = classify_avatar(&stats, &default_table());
        assert!(matches!(tier, AvatarRatingBucket::TierC));
    }
}
