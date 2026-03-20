use crate::config::{AxisChoice, SplitPolicy};

pub const MAX_ZONE_DEPTH: u8 = 6;
pub type EntityId = u64;

#[derive(Debug, Clone, Copy)]
pub struct KdPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl KdPoint {
    pub fn spread(a: &[KdPoint], b: &[KdPoint]) -> (f32, f32, f32) {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for p in a.iter().chain(b.iter()) {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }
        if min_x.is_infinite() {
            return (0.0, 0.0, 0.0);
        }
        (max_x - min_x, max_y - min_y, max_z - min_z)
    }
}

#[derive(Debug, Clone)]
pub struct KdBoundary {
    pub min: KdPoint,
    pub max: KdPoint,
}

impl KdBoundary {
    pub fn axis_span(&self, axis: KdAxis) -> f32 {
        match axis {
            KdAxis::X => self.max.x - self.min.x,
            KdAxis::Y => self.max.y - self.min.y,
            KdAxis::Z => self.max.z - self.min.z,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntitySample {
    pub id: EntityId,
    pub position: KdPoint,
    pub zone_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KdAxis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone)]
pub struct KdTreeNode {
    pub zone_id: String,
    pub bounds: KdBoundary,
    pub axis: KdAxis,
    pub split_value: f32,
    pub depth: u8,
    pub left: Option<Box<KdTreeNode>>,
    pub right: Option<Box<KdTreeNode>>,
    pub entities: Vec<EntityId>,
}

#[derive(Debug)]
pub enum KdTreeSplitResult {
    SplitDone {
        axis: KdAxis,
        value: f32,
        left_count: usize,
        right_count: usize,
    },
    NotEnoughSpread,
}

#[derive(Debug)]
pub struct KdTree {
    pub root: KdTreeNode,
}

impl KdTree {
    pub fn new(root_id: impl Into<String>, boundary: KdBoundary, policy: &SplitPolicy) -> Self {
        let _ = policy.max_depth;
        Self {
            root: KdTreeNode {
                zone_id: root_id.into(),
                bounds: boundary,
                axis: KdAxis::X,
                split_value: 0.0,
                depth: 0,
                left: None,
                right: None,
                entities: Vec::new(),
            },
        }
    }

    pub fn choose_axis(points: &[EntitySample], preferred: &[AxisChoice]) -> KdAxis {
        let kd_points: Vec<KdPoint> = points.iter().map(|s| s.position).collect();
        let (spread_x, spread_y, spread_z) = KdPoint::spread(&kd_points, &[]);
        let candidate = [
            (spread_x, KdAxis::X),
            (spread_y, KdAxis::Y),
            (spread_z, KdAxis::Z),
        ];
        for axis in preferred {
            let selected = match axis {
                AxisChoice::X => KdAxis::X,
                AxisChoice::Y => KdAxis::Y,
                AxisChoice::Z => KdAxis::Z,
            };
            if candidate
                .iter()
                .any(|(span, candidate_axis)| *candidate_axis == selected && *span > 0.1)
            {
                return selected;
            }
        }
        candidate
            .iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, axis)| *axis)
            .unwrap_or(KdAxis::X)
    }

    pub fn split_if_needed(
        &mut self,
        node_id: &str,
        players: &[EntitySample],
        policy: &SplitPolicy,
    ) -> Option<KdTreeSplitResult> {
        if players.len() < 4 || policy.max_depth == 0 {
            return Some(KdTreeSplitResult::NotEnoughSpread);
        }
        let axis = Self::choose_axis(players, &policy.preferred_axes);
        let span = self.root.bounds.axis_span(axis);
        if span <= 0.001 {
            return Some(KdTreeSplitResult::NotEnoughSpread);
        }
        if self.root.zone_id != node_id {
            return None;
        }
        let split_value = match axis {
            KdAxis::X => {
                players.iter().map(|sample| sample.position.x).sum::<f32>() / players.len() as f32
            }
            KdAxis::Y => {
                players.iter().map(|sample| sample.position.y).sum::<f32>() / players.len() as f32
            }
            KdAxis::Z => {
                players.iter().map(|sample| sample.position.z).sum::<f32>() / players.len() as f32
            }
        };
        let (left_count, right_count) = players.iter().fold((0usize, 0usize), |(l, r), sample| {
            let value = match axis {
                KdAxis::X => sample.position.x,
                KdAxis::Y => sample.position.y,
                KdAxis::Z => sample.position.z,
            };
            if value <= split_value {
                (l + 1, r)
            } else {
                (l, r + 1)
            }
        });
        Some(KdTreeSplitResult::SplitDone {
            axis,
            value: split_value,
            left_count,
            right_count,
        })
    }
}
