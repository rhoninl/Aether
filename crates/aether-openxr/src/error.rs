use std::fmt;

/// Errors from OpenXR operations.
#[derive(Debug)]
pub enum OpenXrError {
    InstanceCreation(String),
    SystemNotFound,
    SessionCreation(String),
    SwapchainCreation(String),
    FrameError(String),
    ActionError(String),
    GraphicsError(String),
}

impl fmt::Display for OpenXrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenXrError::InstanceCreation(msg) => {
                write!(f, "OpenXR instance creation failed: {msg}")
            }
            OpenXrError::SystemNotFound => write!(f, "OpenXR system not found (no HMD connected?)"),
            OpenXrError::SessionCreation(msg) => write!(f, "OpenXR session creation failed: {msg}"),
            OpenXrError::SwapchainCreation(msg) => {
                write!(f, "OpenXR swapchain creation failed: {msg}")
            }
            OpenXrError::FrameError(msg) => write!(f, "OpenXR frame error: {msg}"),
            OpenXrError::ActionError(msg) => write!(f, "OpenXR action error: {msg}"),
            OpenXrError::GraphicsError(msg) => write!(f, "OpenXR graphics error: {msg}"),
        }
    }
}

impl std::error::Error for OpenXrError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_instance_creation() {
        let e = OpenXrError::InstanceCreation("no runtime".to_string());
        assert!(e.to_string().contains("no runtime"));
        assert!(e.to_string().contains("instance"));
    }

    #[test]
    fn display_system_not_found() {
        let e = OpenXrError::SystemNotFound;
        assert!(e.to_string().contains("system not found"));
    }

    #[test]
    fn display_session_creation() {
        let e = OpenXrError::SessionCreation("EGL failed".to_string());
        assert!(e.to_string().contains("EGL failed"));
    }

    #[test]
    fn display_swapchain_creation() {
        let e = OpenXrError::SwapchainCreation("bad format".to_string());
        assert!(e.to_string().contains("bad format"));
    }

    #[test]
    fn display_frame_error() {
        let e = OpenXrError::FrameError("timeout".to_string());
        assert!(e.to_string().contains("timeout"));
    }

    #[test]
    fn display_action_error() {
        let e = OpenXrError::ActionError("not attached".to_string());
        assert!(e.to_string().contains("not attached"));
    }

    #[test]
    fn display_graphics_error() {
        let e = OpenXrError::GraphicsError("shader fail".to_string());
        assert!(e.to_string().contains("shader fail"));
    }
}
