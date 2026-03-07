use crate::types::NetEntity;

#[derive(Debug, Clone)]
pub enum NetChannel {
    EntityState,
    PhysicsState,
    AudioState,
    ScriptState,
}

#[derive(Debug, Clone)]
pub struct DeltaState {
    pub entity_id: NetEntity,
    pub channel: NetChannel,
    pub seq: u64,
    pub payload: Vec<u8>,
    pub previous: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct StateDiff {
    pub entity_id: NetEntity,
    pub xor_bytes: Vec<u8>,
    pub len: usize,
}

pub fn xor_patch(base: &[u8], next: &[u8]) -> StateDiff {
    let len = base.len().max(next.len());
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let a = base.get(i).copied().unwrap_or(0);
        let b = next.get(i).copied().unwrap_or(0);
        out.push(a ^ b);
    }
    StateDiff {
        entity_id: NetEntity(0),
        xor_bytes: out.clone(),
        len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_diff_roundtrip_preserves_change() {
        let base = b"abcdefgh";
        let next = b"abcXefGh";
        let diff = xor_patch(base, next);
        let mut reconstructed = base.to_vec();
        for (i, x) in diff.xor_bytes.iter().enumerate() {
            if i < reconstructed.len() {
                reconstructed[i] ^= *x;
            } else {
                reconstructed.push(*x);
            }
        }
        assert_eq!(&reconstructed[..next.len()], next);
        assert_eq!(diff.len, next.len());
    }
}
