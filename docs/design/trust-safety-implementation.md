# Trust & Safety Runtime Controls (task-026)

Added runtime models for safety bubble, visibility, anonymous mode, parental controls, and owner moderation tools.

## Implemented API surface

- Added crate `aether-trust-safety` with controls, visibility, parental, and moderation toolsets.
- Added personal space and anonymous modes.
- Added world owner actions (mute/kick/ban).

## Remaining implementation work

- Implement enforcement hooks inside world simulation loop.
- Add abuse reporting and moderation escalation integrations.
