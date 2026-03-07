---
id: task-001
title: ECS Core Architecture
status: Done
assignee:
  - '@codex-001'
created_date: '2026-03-07 13:16'
updated_date: '2026-03-07 15:11'
labels: []
dependencies: []
priority: high
ordinal: 500
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement the archetype-based Entity Component System (ECS) as the backbone of the Aether engine.

All game objects — avatars, props, terrain, particles, UI elements — are entities composed of data components processed by systems.

Key components: Transform, RigidBody, MeshRenderer, Avatar, NetworkIdentity, Collider, AudioSource, ScriptHost, InterestGroup.

Ref: docs/design/DESIGN.md Section 3.1
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Archetype-based storage with cache-friendly data layout
- [x] #2 Parallel system scheduling with explicit dependency graphs
- [x] #3 System stages: Input → PrePhysics → Physics → PostPhysics → Animation → PreRender → Render → NetworkSync
- [x] #4 Network-aware components (replicated vs server-only)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Phase 1: Project Setup
- Initialize Rust workspace with Cargo
- Create `aether-ecs` crate as the core ECS library

### Phase 2: Core ECS Types
- Entity (generational index)
- Component trait + storage
- Archetype-based storage with cache-friendly layout

### Phase 3: Archetype Storage
- Archetype definition (component type set)
- Archetype table (column-based SoA storage)
- Entity-to-archetype mapping
- Component add/remove (archetype migration)

### Phase 4: System Scheduling
- System trait with queries
- Stage-based pipeline (Input → PrePhysics → Physics → PostPhysics → Animation → PreRender → Render → NetworkSync)
- Dependency graph for parallel execution
- Query system with read/write access tracking

### Phase 5: Network-Aware Components
- Replication markers (Replicated vs ServerOnly)
- NetworkIdentity component

### Phase 6: Tests
- Write comprehensive tests covering all acceptance criteria
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Complete (claude-001)

### What was built:
- **Entity system**: Generational indices with O(1) alive check and O(1) len()
- **Component system**: TypeId-based ComponentId, registry with drop fn tracking, ReplicationMode (Replicated/ServerOnly)
- **Archetype storage**: SoA column-based storage, archetype migration on add/remove, ZST support
- **Query system**: AccessDescriptor with read/write tracking, conflict detection, archetype filtering
- **System scheduler**: Stage-based pipeline (8 stages), greedy parallel batching via rayon, metrics/alerting
- **Network components**: NetworkIdentity, Authority, replicated/server-only filtering
- **World**: Spawn/despawn, component CRUD, archetype migration, system execution

### Critical bugs fixed during code review:
1. Column::swap_remove drop semantics split into drop_at + swap_remove (prevents double-free/leak)
2. run_systems aliasing fixed via std::mem::take (prevents UB)
3. remove_component now explicitly drops the removed component
4. EntityAllocator::len() changed from O(n) to O(1)
5. ZST support fixed with dangling pointer for zero-sized columns

### Test coverage: 99 tests covering:
- Entity allocation/deallocation/generation
- Component registration/ID/replication
- Archetype storage/migration/swap-remove
- Query access/conflict/matching
- System scheduling/stages/parallelism/metrics
- Network component filtering
- Drop correctness (no double-free, no leaks)
- ZST components
- Scale test (200 entities, column grow)
- Edge cases (dead entities, missing components, sequential migrations)
<!-- SECTION:NOTES:END -->
