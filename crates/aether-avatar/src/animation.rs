#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProceduralStateMachine {
    Idle,
    Locomote,
    Gesture,
    GestureRecover,
    Fall,
}

#[derive(Debug, Clone)]
pub enum ProceduralGesture {
    Wave,
    Point,
    PointAndHold,
    Victory,
    Dance,
    Nod,
}

#[derive(Debug, Clone)]
pub enum LocomotionIntent {
    Stationary,
    Walk,
    Sprint,
    Crouch,
    Jump,
    FallRecovery,
}

#[derive(Debug, Clone)]
pub struct BlendPoint {
    pub state: ProceduralStateMachine,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct BlendCurve {
    pub target_state: ProceduralStateMachine,
    pub from_weight: f32,
    pub to_weight: f32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct BlendTransition {
    pub from: ProceduralStateMachine,
    pub to: ProceduralStateMachine,
    pub reason: ProceduralGesture,
    pub curve: BlendCurve,
}

#[derive(Debug, Clone)]
pub enum BlendTransitionKind {
    Linear,
    SmoothStep,
}

#[derive(Debug, Clone)]
pub struct BlendStateInput {
    pub gesture: Option<ProceduralGesture>,
    pub locomotion: LocomotionIntent,
    pub in_air: bool,
}
