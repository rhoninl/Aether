/// Description of a joint connecting two rigid bodies.
#[derive(Debug, Clone, PartialEq)]
pub enum JointType {
    /// A fixed joint that locks two bodies together.
    Fixed {
        anchor1: [f32; 3],
        anchor2: [f32; 3],
    },
    /// A revolute (hinge) joint that allows rotation around an axis.
    Revolute {
        axis: [f32; 3],
        anchor1: [f32; 3],
        anchor2: [f32; 3],
    },
    /// A prismatic (slider) joint that allows translation along an axis.
    Prismatic {
        axis: [f32; 3],
        anchor1: [f32; 3],
        anchor2: [f32; 3],
        limits: Option<[f32; 2]>,
    },
}

impl JointType {
    /// Create a fixed joint with the given local anchors.
    pub fn fixed(anchor1: [f32; 3], anchor2: [f32; 3]) -> Self {
        JointType::Fixed { anchor1, anchor2 }
    }

    /// Create a revolute joint with the given axis and local anchors.
    pub fn revolute(axis: [f32; 3], anchor1: [f32; 3], anchor2: [f32; 3]) -> Self {
        JointType::Revolute {
            axis,
            anchor1,
            anchor2,
        }
    }

    /// Create a prismatic joint with the given axis, local anchors, and optional limits.
    pub fn prismatic(
        axis: [f32; 3],
        anchor1: [f32; 3],
        anchor2: [f32; 3],
        limits: Option<[f32; 2]>,
    ) -> Self {
        JointType::Prismatic {
            axis,
            anchor1,
            anchor2,
            limits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_joint_creation() {
        let joint = JointType::fixed([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
        match joint {
            JointType::Fixed { anchor1, anchor2 } => {
                assert_eq!(anchor1, [1.0, 0.0, 0.0]);
                assert_eq!(anchor2, [-1.0, 0.0, 0.0]);
            }
            _ => panic!("Expected Fixed joint"),
        }
    }

    #[test]
    fn revolute_joint_creation() {
        let joint = JointType::revolute([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
        match joint {
            JointType::Revolute {
                axis,
                anchor1,
                anchor2,
            } => {
                assert_eq!(axis, [0.0, 0.0, 1.0]);
                assert_eq!(anchor1, [1.0, 0.0, 0.0]);
                assert_eq!(anchor2, [-1.0, 0.0, 0.0]);
            }
            _ => panic!("Expected Revolute joint"),
        }
    }

    #[test]
    fn prismatic_joint_creation_with_limits() {
        let joint = JointType::prismatic(
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            Some([-1.0, 1.0]),
        );
        match joint {
            JointType::Prismatic {
                axis,
                anchor1,
                anchor2,
                limits,
            } => {
                assert_eq!(axis, [0.0, 1.0, 0.0]);
                assert_eq!(anchor1, [0.0, 0.0, 0.0]);
                assert_eq!(anchor2, [0.0, 0.0, 0.0]);
                assert_eq!(limits, Some([-1.0, 1.0]));
            }
            _ => panic!("Expected Prismatic joint"),
        }
    }

    #[test]
    fn prismatic_joint_without_limits() {
        let joint = JointType::prismatic([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], None);
        match joint {
            JointType::Prismatic { limits, .. } => {
                assert!(limits.is_none());
            }
            _ => panic!("Expected Prismatic joint"),
        }
    }

    #[test]
    fn joint_type_equality() {
        let a = JointType::fixed([0.0; 3], [0.0; 3]);
        let b = JointType::fixed([0.0; 3], [0.0; 3]);
        assert_eq!(a, b);
    }

    #[test]
    fn different_joint_types_not_equal() {
        let fixed = JointType::fixed([0.0; 3], [0.0; 3]);
        let revolute = JointType::revolute([0.0, 0.0, 1.0], [0.0; 3], [0.0; 3]);
        assert_ne!(fixed, revolute);
    }
}
