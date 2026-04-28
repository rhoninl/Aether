//! Runtime events surfaced via `XrInstance::poll_events` (design doc §6).
//!
//! Each event maps to one `XrEventDataBuffer` variant the OpenXR runtime
//! produces. The HAL deliberately does not expose the raw struct: applications
//! drive their state machine off this enum, and backends do the C-side
//! translation.

use crate::instance::ExtensionId;
use crate::session::SessionState;

#[derive(Debug, Clone, PartialEq)]
pub enum XrEvent {
    /// `XR_TYPE_EVENT_DATA_SESSION_STATE_CHANGED`.
    SessionStateChanged { state: SessionState },
    /// `XR_TYPE_EVENT_DATA_INSTANCE_LOSS_PENDING`.
    InstanceLossPending,
    /// `XR_TYPE_EVENT_DATA_INTERACTION_PROFILE_CHANGED`.
    InteractionProfileChanged,
    /// `XR_TYPE_EVENT_DATA_REFERENCE_SPACE_CHANGE_PENDING`.
    ReferenceSpaceChangePending,
    /// `XR_TYPE_EVENT_DATA_EVENTS_LOST`.
    EventsLost { lost_count: u32 },
    /// Extension-specific event the HAL doesn't understand. Carries the
    /// extension that defined it so the application can decide whether to
    /// react.
    Unknown { extension: Option<ExtensionId> },
}
