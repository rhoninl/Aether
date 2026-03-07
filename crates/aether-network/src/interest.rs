use crate::types::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bucket {
    Dormant = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

#[derive(Debug, Clone, Copy)]
pub struct InterestPolicy {
    pub critical_distance_m: f32,
    pub high_distance_m: f32,
    pub medium_distance_m: f32,
    pub low_distance_m: f32,
}

impl Default for InterestPolicy {
    fn default() -> Self {
        Self {
            critical_distance_m: 8.0,
            high_distance_m: 18.0,
            medium_distance_m: 48.0,
            low_distance_m: 120.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CameraFrustum {
    pub fov_deg: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraFrustum {
    fn default() -> Self {
        Self {
            fov_deg: 90.0,
            aspect: 1.8,
            near: 0.2,
            far: 800.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientProfile {
    pub client_id: u64,
    pub world_id: u64,
    pub position: Vec3,
    pub frustum: CameraFrustum,
}

#[derive(Debug, Clone)]
pub struct ClientBudget {
    pub client_id: u64,
    pub max_entities: usize,
    pub max_bytes_per_tick: usize,
}

pub struct InterestManager {
    policy: InterestPolicy,
}

impl InterestManager {
    pub fn new(policy: InterestPolicy) -> Self {
        Self { policy }
    }

    pub fn bucket_by_distance(&self, distance: f32) -> Bucket {
        if distance <= self.policy.critical_distance_m {
            Bucket::Critical
        } else if distance <= self.policy.high_distance_m {
            Bucket::High
        } else if distance <= self.policy.medium_distance_m {
            Bucket::Medium
        } else if distance <= self.policy.low_distance_m {
            Bucket::Low
        } else {
            Bucket::Dormant
        }
    }

    pub fn top_n_entities(
        &self,
        candidates: &[(u64, Vec3, f32)],
        budget: &ClientBudget,
        viewer_pos: Vec3,
    ) -> Vec<u64> {
        let mut sorted = Vec::new();
        for (id, pos, _importance) in candidates {
            let distance = (pos.x - viewer_pos.x).abs() + (pos.y - viewer_pos.y).abs() + (pos.z - viewer_pos.z).abs();
            let bucket = self.bucket_by_distance(distance);
            let priority = (bucket as i32) * 1_000_000 + (1_000.0 - distance).max(0.0) as i32;
            sorted.push((priority, *id));
        }
        sorted.sort_by(|a, b| b.0.cmp(&a.0));
        sorted
            .into_iter()
            .take(budget.max_entities)
            .map(|(_, id)| id)
            .collect()
    }

    pub fn frustum_visible(_frustum: &CameraFrustum, _position: Vec3, _target: Vec3) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_buckets_promote_and_demote() {
        let mgr = InterestManager::new(InterestPolicy::default());
        let b = mgr.bucket_by_distance(4.0);
        assert!(matches!(b, Bucket::Critical));
        assert!(matches!(mgr.bucket_by_distance(9.0), Bucket::High));
        assert!(matches!(mgr.bucket_by_distance(30.0), Bucket::Medium));
    }

    #[test]
    fn client_budget_selects_top_entities() {
        let mgr = InterestManager::new(InterestPolicy::default());
        let budget = ClientBudget {
            client_id: 1,
            max_entities: 2,
            max_bytes_per_tick: 1024,
        };
        let viewer = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
        let candidates = vec![
            (1, Vec3 { x: 1.0, y: 0.0, z: 0.0 }, 1.0),
            (2, Vec3 { x: 2.0, y: 0.0, z: 0.0 }, 1.0),
            (3, Vec3 { x: 3.0, y: 0.0, z: 0.0 }, 1.0),
        ];
        let selected = mgr.top_n_entities(&candidates, &budget, viewer);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0], 1);
    }
}
