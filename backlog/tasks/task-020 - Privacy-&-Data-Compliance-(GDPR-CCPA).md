---
id: task-020
title: Privacy & Data Compliance (GDPR/CCPA)
status: To Do
assignee: []
created_date: '2026-03-07 13:19'
updated_date: '2026-03-07 14:13'
labels: []
dependencies:
  - task-012
  - task-011
  - task-013
priority: medium
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement privacy controls: account deletion with ledger pseudonymization, data export, Compliance Keystore for legal holds, and 7-year retention schedule.

Ref: docs/design/DESIGN.md Section 6.3
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Account deletion: profile/social/chat/telemetry fully deleted
- [ ] #2 Ledger pseudonymization: SHA-256(user_id + deletion_salt)
- [ ] #3 Compliance Keystore: encrypted, dual-approval, audit-logged salt storage
- [ ] #4 Legal hold support: defer deletion for active investigations
- [ ] #5 GDPR Article 17(3)(b) legal basis for financial record retention
- [ ] #6 7-year retention then permanent deletion of rows + salt
- [ ] #7 Data export (Article 20) before pseudonymization
<!-- AC:END -->
