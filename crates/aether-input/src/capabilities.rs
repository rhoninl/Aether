#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputBackend {
    OpenXr,
    OculusSdk,
    SteamVr,
    Pico,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerType {
    HandsOnly,
    QuestControllers,
    ViveWands,
    IndexControllers,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputActionPath {
    LeftHand,
    RightHand,
    HMD,
    LeftFoot,
    RightFoot,
    FullBody,
}

#[derive(Debug, Clone)]
pub enum Capability {
    HandTracking,
    ControllerInput,
    EyeGaze,
    FaceTracking,
    VoiceChat,
}

#[derive(Debug)]
pub enum CapabilityError {
    MissingCapability(Capability),
    UnsupportedBackend(InputBackend),
    DeviceDisconnected,
}

#[derive(Debug)]
pub struct HeadsetProfile {
    pub backend: InputBackend,
    pub controller: ControllerType,
    pub supported: Vec<Capability>,
    pub max_touch_points: u8,
}

#[derive(Debug, Clone)]
pub struct InputFrameHint {
    pub backend: InputBackend,
    pub session_id: String,
    pub capabilities: Vec<Capability>,
}
