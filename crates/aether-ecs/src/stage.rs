/// The fixed stages of the ECS pipeline, executed in order each tick.
///
/// Systems are assigned to stages. Within a stage, systems run in parallel
/// where access patterns allow. Stages execute sequentially.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stage {
    Input = 0,
    PrePhysics = 1,
    Physics = 2,
    PostPhysics = 3,
    Animation = 4,
    PreRender = 5,
    Render = 6,
    NetworkSync = 7,
}

impl Stage {
    /// All stages in execution order.
    pub const ALL: [Stage; 8] = [
        Stage::Input,
        Stage::PrePhysics,
        Stage::Physics,
        Stage::PostPhysics,
        Stage::Animation,
        Stage::PreRender,
        Stage::Render,
        Stage::NetworkSync,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Input => "Input",
            Stage::PrePhysics => "PrePhysics",
            Stage::Physics => "Physics",
            Stage::PostPhysics => "PostPhysics",
            Stage::Animation => "Animation",
            Stage::PreRender => "PreRender",
            Stage::Render => "Render",
            Stage::NetworkSync => "NetworkSync",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_in_correct_order() {
        let stages = Stage::ALL;
        for i in 1..stages.len() {
            assert!(stages[i - 1] < stages[i], "Stages must be in order");
        }
    }

    #[test]
    fn all_stages_present() {
        assert_eq!(Stage::ALL.len(), 8);
    }

    #[test]
    fn stage_names() {
        assert_eq!(Stage::Input.name(), "Input");
        assert_eq!(Stage::Physics.name(), "Physics");
        assert_eq!(Stage::Render.name(), "Render");
        assert_eq!(Stage::NetworkSync.name(), "NetworkSync");
    }

    #[test]
    fn stage_ordering() {
        assert!(Stage::Input < Stage::PrePhysics);
        assert!(Stage::PrePhysics < Stage::Physics);
        assert!(Stage::Physics < Stage::PostPhysics);
        assert!(Stage::PostPhysics < Stage::Animation);
        assert!(Stage::Animation < Stage::PreRender);
        assert!(Stage::PreRender < Stage::Render);
        assert!(Stage::Render < Stage::NetworkSync);
    }
}
