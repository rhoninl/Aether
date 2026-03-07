use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialBatchKey {
    pub mesh_id: u64,
    pub material_id: u64,
    pub pass_id: u8,
    pub blend_mode: u8,
}

#[derive(Debug, Clone)]
pub struct BatchRequest {
    pub entity_id: u64,
    pub key: MaterialBatchKey,
}

#[derive(Debug, Clone)]
pub struct BatchHint {
    pub key: MaterialBatchKey,
    pub instances: Vec<u64>,
}

pub fn batch_instances_by_key(requests: &[BatchRequest]) -> Vec<BatchHint> {
    let mut map: HashMap<MaterialBatchKey, Vec<u64>> = HashMap::new();
    for req in requests {
        map.entry(req.key).or_default().push(req.entity_id);
    }

    let mut grouped: Vec<BatchHint> = map
        .into_iter()
        .map(|(key, instances)| BatchHint { key, instances })
        .collect();

    grouped.sort_by(|a, b| {
        (
            a.key.material_id,
            a.key.mesh_id,
            a.key.pass_id,
            a.key.blend_mode,
        )
            .cmp(&(
                b.key.material_id,
                b.key.mesh_id,
                b.key.pass_id,
                b.key.blend_mode,
            ))
    });
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_hint_grouping_and_ordering_is_deterministic() {
        let keys = [
            MaterialBatchKey {
                mesh_id: 10,
                material_id: 2,
                pass_id: 1,
                blend_mode: 0,
            },
            MaterialBatchKey {
                mesh_id: 8,
                material_id: 1,
                pass_id: 1,
                blend_mode: 0,
            },
            MaterialBatchKey {
                mesh_id: 8,
                material_id: 1,
                pass_id: 1,
                blend_mode: 0,
            },
        ];
        let requests = vec![
            BatchRequest {
                entity_id: 101,
                key: keys[0],
            },
            BatchRequest {
                entity_id: 102,
                key: keys[1],
            },
            BatchRequest {
                entity_id: 103,
                key: keys[2],
            },
        ];
        let out = batch_instances_by_key(&requests);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].key.material_id, 1);
        assert_eq!(out[0].instances, vec![102, 103]);
        assert_eq!(out[1].key.material_id, 2);
    }
}
