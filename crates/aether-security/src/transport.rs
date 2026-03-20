#[derive(Debug, Clone)]
pub enum ClientAddress {
    Ipv4(String),
    Ipv6(String),
}

#[derive(Debug)]
pub enum NetworkAction {
    Connect,
    Frame,
    RpcCall,
    ChatSend,
    AssetFetch,
}

#[derive(Debug)]
pub enum AttackSignal {
    BurstTraffic,
    SuspiciousHandshake,
    MalformedPacket,
}

#[derive(Debug)]
pub enum FloodSignal {
    PingFlood,
    PacketFlood,
    HandshakeLoop,
}

#[derive(Debug)]
pub enum DdosDefenseState {
    Calm,
    Throttle,
    Challenge,
    Blackhole,
}
