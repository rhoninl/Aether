//! Composition layer recording (design doc §5.4, §8, P2-B).
//!
//! V1 supports `XrCompositionLayerProjection` only. Quad/cylinder/cube/passthrough
//! layers are deferred (§8) but the builder API is shaped so they can be added
//! later without breaking the trait surface.

use crate::swapchain::SwapchainImageIndex;
use crate::view::View;

/// One projection-layer view (`XrCompositionLayerProjectionView`): which eye,
/// which view pose+fov, and which swapchain image holds the rendered pixels.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionLayerView {
    pub eye_index: u32,
    pub view: View,
    pub swapchain_image: SwapchainImageIndex,
}

/// Result of finishing a [`LayerBuilder`]. Passed verbatim to
/// `XrFrame::end()`; backends submit it as `XrCompositionLayerProjection` (one
/// per submission in V1).
#[derive(Debug, Clone, Default)]
pub struct LayerSubmission {
    pub projection_views: Vec<ProjectionLayerView>,
}

/// Per-frame layer recorder. Borrowed from the frame so views can't outlive
/// the swapchain images they reference.
pub struct LayerBuilder<'frame> {
    submission: LayerSubmission,
    _frame: std::marker::PhantomData<&'frame mut ()>,
}

impl<'frame> LayerBuilder<'frame> {
    /// Create an empty builder. Backends call this from `XrFrame::begin()`.
    pub fn new() -> Self {
        Self {
            submission: LayerSubmission::default(),
            _frame: std::marker::PhantomData,
        }
    }

    pub fn add_projection_layer(
        &mut self,
        eye_index: u32,
        view: View,
        swapchain_image: SwapchainImageIndex,
    ) -> &mut Self {
        self.submission.projection_views.push(ProjectionLayerView {
            eye_index,
            view,
            swapchain_image,
        });
        self
    }

    pub fn finish(self) -> LayerSubmission {
        self.submission
    }
}

impl<'frame> Default for LayerBuilder<'frame> {
    fn default() -> Self {
        Self::new()
    }
}
