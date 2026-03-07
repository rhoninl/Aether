---
id: task-015
title: Deployment Infrastructure
status: In Progress
assignee: []
created_date: '2026-03-07 13:18'
updated_date: '2026-03-07 15:00'
labels: []
dependencies: []
priority: medium
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Set up multi-region Kubernetes deployment: PostgreSQL + Citus (single-primary economy), NATS supercluster, Redis, MinIO, CDN, Patroni failover, and auto-scaling.

Ref: docs/design/DESIGN.md Section 4.3
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Multi-region K8s clusters (US-West, EU-Central, Asia-East)
- [ ] #2 PostgreSQL + Citus: single-primary for economy, sharded for social/registry
- [ ] #3 Patroni-based failover for economy primary (< 30s)
- [ ] #4 NATS JetStream supercluster for inter-service events
- [ ] #5 Redis for cache, presence, leaderboards
- [ ] #6 MinIO/S3 for asset blob storage
- [ ] #7 CDN with 50+ edge PoPs for asset delivery
- [ ] #8 Custom HPA auto-scaling for world servers
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add deployment manifest crate (`aether-deploy`) for topology/policy definitions (regions, clusters, failover, autoscale).
2. Add infra component descriptors for Postgres/Citus, NATS, Redis, MinIO, CDN, HPA.
3. Add config/schema objects to generate or validate deployment recipes.
4. Add operations-oriented documentation of region, failover, and scaling contracts.
<!-- SECTION:PLAN:END -->
