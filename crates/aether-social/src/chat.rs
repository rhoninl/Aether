#[derive(Debug, Clone)]
pub enum ChatType {
    DirectMessage,
    Group,
    World,
    SpatialVoice,
}

#[derive(Debug, Clone)]
pub enum MessageKind {
    Text(String),
    SystemAnnouncement(String),
    Emote(String),
    Command(String),
}

#[derive(Debug, Clone)]
pub struct ChatChannel {
    pub channel_id: String,
    pub kind: ChatType,
    pub members: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_id: String,
    pub from_user: u64,
    pub channel_id: String,
    pub kind: MessageKind,
    pub server_ts_ms: u64,
}

