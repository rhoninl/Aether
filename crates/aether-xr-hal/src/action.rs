//! Typed action system (design doc §5.6, §7).
//!
//! - [`ActionValue`] is a sealed marker trait identifying which OpenXR action
//!   value types we support: `bool`, `f32`, `[f32; 2]` (2D axis), and `Pose3`.
//! - [`XrAction<T>`] is the per-action handle the application queries each frame.
//! - [`XrActionSet`] is an opaque handle to a set attached to a session via
//!   `xrAttachSessionActionSets`.
//! - [`ActionManifest`] is the declarative builder applications use to describe
//!   their action sets ahead of time; the HAL turns a manifest into the typed
//!   `XrAction<T>` handles.

use crate::frame::XrFrame;
use crate::profile::{BindingPath, InteractionProfile};
use crate::tracking::Pose3;

mod sealed {
    /// Sealed-trait pattern: only this crate may implement [`ActionValue`].
    pub trait Sealed {}
}

/// Marker trait for values an [`XrAction`] can carry.
///
/// Sealed: the impls are limited to the OpenXR-defined action value types
/// (`bool`, `f32`, `[f32; 2]`, `Pose3`). New value types must be added here
/// rather than by downstream crates so the HAL keeps full control of the OpenXR
/// type-mapping.
pub trait ActionValue: sealed::Sealed + Sized + Clone + 'static {}

impl sealed::Sealed for bool {}
impl ActionValue for bool {}

impl sealed::Sealed for f32 {}
impl ActionValue for f32 {}

impl sealed::Sealed for [f32; 2] {}
impl ActionValue for [f32; 2] {}

impl sealed::Sealed for Pose3 {}
impl ActionValue for Pose3 {}

/// Snapshot of an action's value at a given sync point.
///
/// Mirrors `XrActionStateBoolean`/`Float`/`Vector2f`/`Pose`: the latest value
/// plus the bookkeeping flags the runtime returns alongside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionState<T> {
    /// The current value at the last `xrSyncActions` call.
    pub current: T,
    /// True if the value changed since the previous sync.
    pub changed_since_last_sync: bool,
    /// True if the runtime considers the action bound and active.
    pub is_active: bool,
}

/// Opaque handle to an action set after creation. Sets are referenced by handle
/// when attaching to a session and when syncing per-frame so the underlying
/// runtime resource is never moved or copied across the API boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionSetHandle(pub u32);

/// Opaque action-set object. Backends own the underlying `XrActionSet` runtime
/// handle; this struct is the value carried across the HAL.
#[derive(Debug, Clone)]
pub struct XrActionSet {
    handle: ActionSetHandle,
    name: String,
}

impl XrActionSet {
    /// Construct an action-set wrapper. Backends call this once they have created
    /// the underlying `XrActionSet`.
    pub fn new(handle: ActionSetHandle, name: impl Into<String>) -> Self {
        Self {
            handle,
            name: name.into(),
        }
    }

    pub fn handle(&self) -> ActionSetHandle {
        self.handle
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A typed action handle.
///
/// `T` is constrained by the sealed [`ActionValue`] trait so an action's value
/// type can never drift from the OpenXR-supported set at compile time.
pub trait XrAction<T: ActionValue> {
    /// Action name, as registered in the manifest.
    fn name(&self) -> &str;

    /// Read the action's current state. Must be called after the per-frame
    /// `XrFrame::sync_actions` to see fresh values.
    fn current(&self, frame: &impl XrFrame) -> ActionState<T>;

    /// Suggest controller bindings for an interaction profile. Maps to
    /// `xrSuggestInteractionProfileBindings`.
    fn suggest_bindings(&self, profile: InteractionProfile, paths: &[BindingPath]);
}

/// Declarative description of one action inside an [`ActionManifest`].
///
/// Action types are encoded as a fieldless enum rather than as a generic
/// parameter so a single manifest can declare a heterogeneous mix of actions
/// (matching the OpenXR `XrActionType`).
#[derive(Debug, Clone)]
pub struct ActionDecl {
    pub name: String,
    pub localized_name: String,
    pub kind: ActionKind,
    pub suggested: Vec<(InteractionProfile, Vec<BindingPath>)>,
}

/// OpenXR action value kind. Mirrors `XrActionType`; the trait surface uses the
/// typed [`ActionValue`] impls, while the manifest uses this discriminator so
/// declarations can be serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionKind {
    /// `XR_ACTION_TYPE_BOOLEAN_INPUT`
    Boolean,
    /// `XR_ACTION_TYPE_FLOAT_INPUT`
    Float,
    /// `XR_ACTION_TYPE_VECTOR2F_INPUT`
    Vector2,
    /// `XR_ACTION_TYPE_POSE_INPUT`
    Pose,
    /// `XR_ACTION_TYPE_VIBRATION_OUTPUT`
    HapticVibration,
}

/// Declarative builder for an action set.
///
/// Applications describe their input surface up front via a manifest; backends
/// turn a manifest into one or more concrete `XrActionSet` + `XrAction<T>`
/// handles. Building the manifest is a pure value operation, so it can be
/// constructed in tests and tooling without a live XR runtime.
#[derive(Debug, Clone, Default)]
pub struct ActionManifest {
    name: String,
    localized_name: String,
    priority: u32,
    actions: Vec<ActionDecl>,
}

impl ActionManifest {
    /// Start a new manifest for the named action set. `priority` corresponds to
    /// `XrActionSetCreateInfo::priority` (higher value wins binding conflicts).
    pub fn new(name: impl Into<String>, localized_name: impl Into<String>, priority: u32) -> Self {
        Self {
            name: name.into(),
            localized_name: localized_name.into(),
            priority,
            actions: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn localized_name(&self) -> &str {
        &self.localized_name
    }

    pub fn priority(&self) -> u32 {
        self.priority
    }

    pub fn actions(&self) -> &[ActionDecl] {
        &self.actions
    }

    /// Declare a typed action. The closure receives an [`ActionBuilder`] so
    /// per-profile suggested bindings can be added inline.
    pub fn action<F>(mut self, name: impl Into<String>, kind: ActionKind, build: F) -> Self
    where
        F: FnOnce(ActionBuilder) -> ActionBuilder,
    {
        let builder = ActionBuilder::new(name.into(), kind);
        self.actions.push(build(builder).finish());
        self
    }
}

/// Per-action builder used inside [`ActionManifest::action`].
pub struct ActionBuilder {
    decl: ActionDecl,
}

impl ActionBuilder {
    fn new(name: String, kind: ActionKind) -> Self {
        Self {
            decl: ActionDecl {
                localized_name: name.clone(),
                name,
                kind,
                suggested: Vec::new(),
            },
        }
    }

    pub fn localized(mut self, localized: impl Into<String>) -> Self {
        self.decl.localized_name = localized.into();
        self
    }

    /// Add suggested bindings for a single interaction profile. May be called
    /// multiple times; each call records one profile entry.
    pub fn binding(mut self, profile: InteractionProfile, paths: &[&str]) -> Self {
        self.decl.suggested.push((
            profile,
            paths.iter().map(|p| BindingPath::from(*p)).collect(),
        ));
        self
    }

    fn finish(self) -> ActionDecl {
        self.decl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time check: ActionValue is implemented for every type the design
    // doc §5.6 names, and only for those types.
    fn assert_action_value<T: ActionValue>() {}

    #[test]
    fn sealed_action_value_impls() {
        assert_action_value::<bool>();
        assert_action_value::<f32>();
        assert_action_value::<[f32; 2]>();
        assert_action_value::<Pose3>();
    }

    #[test]
    fn manifest_builder_collects_actions_and_bindings() {
        let manifest = ActionManifest::new("gameplay", "Gameplay", 0)
            .action("teleport", ActionKind::Boolean, |a| {
                a.localized("Teleport").binding(
                    InteractionProfile::Touch,
                    &["/user/hand/left/input/trigger/click"],
                )
            })
            .action("aim_pose", ActionKind::Pose, |a| {
                a.binding(
                    InteractionProfile::Index,
                    &["/user/hand/right/input/aim/pose"],
                )
            });

        assert_eq!(manifest.name(), "gameplay");
        assert_eq!(manifest.priority(), 0);
        assert_eq!(manifest.actions().len(), 2);

        let teleport = &manifest.actions()[0];
        assert_eq!(teleport.name, "teleport");
        assert_eq!(teleport.localized_name, "Teleport");
        assert_eq!(teleport.kind, ActionKind::Boolean);
        assert_eq!(teleport.suggested.len(), 1);
        assert_eq!(teleport.suggested[0].0, InteractionProfile::Touch);
        assert_eq!(
            teleport.suggested[0].1[0].as_str(),
            "/user/hand/left/input/trigger/click"
        );
    }

    #[test]
    fn xr_action_set_exposes_handle_and_name() {
        let set = XrActionSet::new(ActionSetHandle(42), "gameplay");
        assert_eq!(set.handle(), ActionSetHandle(42));
        assert_eq!(set.name(), "gameplay");
    }

    #[test]
    fn action_state_is_copyable() {
        let s = ActionState {
            current: 0.75_f32,
            changed_since_last_sync: true,
            is_active: true,
        };
        let s2 = s;
        assert!((s2.current - 0.75).abs() < f32::EPSILON);
    }
}
