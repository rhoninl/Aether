# Deployment Infrastructure (task-015)

Added infrastructure policy scaffolding for multi-region topology, databases, eventing, caching, storage, and autoscaling.

## Implemented API surface

- Added crate `aether-deploy` with modules:
  - `catalog`: regions/datacenters/environment topology.
  - `components`: data-plane component descriptors for DB/cache/message bus/storage.
  - `failover`: Patroni-like failover policy models.
  - `k8s`: autoscale and world runtime profiles.
- Updated workspace members to include `aether-deploy`.

## Mapping to acceptance criteria

- `#1` Topology includes multi-region descriptors.
- `#2` Database types allow single-primary + sharded mode representation.
- `#3` Patroni-like failover values in `PatroniConfig` and `DatabaseFailoverPolicy`.
- `#4` Message bus supercluster flag in `MessageBus`.
- `#5` Redis-like cache descriptors via `Cache`.
- `#6` Asset storage descriptor via `AssetStorage`.
- `#7` Edge/cdn and replication fields represented by region/dc and storage flags.
- `#8` Custom autoscale profile and world runtime settings via `AutoscalePolicy`/`HpaProfile`.

## Remaining implementation work

- Emit concrete Kubernetes resources/helm templates and validation schema.
- Connect topology values to deployment controller and runbook tooling.
