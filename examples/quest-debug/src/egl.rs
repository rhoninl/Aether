//! EGL context setup for Android OpenXR.
//!
//! Creates a minimal EGL display + context suitable for headless GLES rendering
//! (OpenXR provides the actual render targets via swapchain).

/// Raw EGL types needed for OpenXR session creation.
#[derive(Debug, Clone, Copy)]
pub struct EglContext {
    pub display: usize,
    pub config: usize,
    pub context: usize,
}

// EGL constants
const EGL_DEFAULT_DISPLAY: usize = 0;
const EGL_NO_CONTEXT: usize = 0;
const EGL_OPENGL_ES3_BIT: i32 = 0x0040;
const EGL_RENDERABLE_TYPE: i32 = 0x3040;
const EGL_BLUE_SIZE: i32 = 0x3022;
const EGL_GREEN_SIZE: i32 = 0x3023;
const EGL_RED_SIZE: i32 = 0x3024;
const EGL_DEPTH_SIZE: i32 = 0x3025;
const EGL_NONE: i32 = 0x3038;
const EGL_CONTEXT_MAJOR_VERSION: i32 = 0x3098;
const EGL_TRUE: i32 = 1;

// EGL function signatures
type EglGetDisplay = unsafe extern "C" fn(usize) -> usize;
type EglInitialize = unsafe extern "C" fn(usize, *mut i32, *mut i32) -> i32;
type EglChooseConfig = unsafe extern "C" fn(usize, *const i32, *mut usize, i32, *mut i32) -> i32;
type EglCreateContext =
    unsafe extern "C" fn(usize, usize, usize, *const i32) -> usize;

/// Initialize EGL for OpenXR on Android.
///
/// Returns EGL display, config, and context handles needed for
/// `xrCreateSession` with the OpenGL ES graphics binding.
pub fn init_egl() -> Result<EglContext, String> {
    unsafe {
        let lib = dlopen(b"libEGL.so\0".as_ptr() as _, 1);
        if lib.is_null() {
            return Err("failed to load libEGL.so".to_string());
        }

        let get_display: EglGetDisplay = load_fn(lib, b"eglGetDisplay\0")?;
        let initialize: EglInitialize = load_fn(lib, b"eglInitialize\0")?;
        let choose_config: EglChooseConfig = load_fn(lib, b"eglChooseConfig\0")?;
        let create_context: EglCreateContext = load_fn(lib, b"eglCreateContext\0")?;

        // Get display
        let display = get_display(EGL_DEFAULT_DISPLAY);
        if display == 0 {
            return Err("eglGetDisplay failed".to_string());
        }

        // Initialize
        let mut major: i32 = 0;
        let mut minor: i32 = 0;
        if initialize(display, &mut major, &mut minor) != EGL_TRUE {
            return Err("eglInitialize failed".to_string());
        }
        log::info!("EGL initialized: {major}.{minor}");

        // Choose config
        let config_attribs = [
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_ES3_BIT,
            EGL_RED_SIZE,
            8,
            EGL_GREEN_SIZE,
            8,
            EGL_BLUE_SIZE,
            8,
            EGL_DEPTH_SIZE,
            0,
            EGL_NONE,
        ];
        let mut config: usize = 0;
        let mut num_configs: i32 = 0;
        if choose_config(
            display,
            config_attribs.as_ptr(),
            &mut config,
            1,
            &mut num_configs,
        ) != EGL_TRUE
            || num_configs == 0
        {
            return Err("eglChooseConfig failed".to_string());
        }

        // Create context
        let context_attribs = [EGL_CONTEXT_MAJOR_VERSION, 3, EGL_NONE];
        let context = create_context(
            display,
            config,
            EGL_NO_CONTEXT,
            context_attribs.as_ptr(),
        );
        if context == 0 {
            return Err("eglCreateContext failed".to_string());
        }

        log::info!("EGL context created");

        Ok(EglContext {
            display,
            config,
            context,
        })
    }
}

unsafe fn load_fn<T>(lib: *mut std::ffi::c_void, name: &[u8]) -> Result<T, String> {
    let sym = dlsym(lib, name.as_ptr() as _);
    if sym.is_null() {
        return Err(format!(
            "failed to load {}",
            std::str::from_utf8(&name[..name.len() - 1]).unwrap_or("?")
        ));
    }
    Ok(std::mem::transmute_copy(&sym))
}

extern "C" {
    fn dlopen(filename: *const std::ffi::c_char, flags: i32) -> *mut std::ffi::c_void;
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const std::ffi::c_char)
        -> *mut std::ffi::c_void;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egl_context_is_copy() {
        let ctx = EglContext {
            display: 1,
            config: 2,
            context: 3,
        };
        let ctx2 = ctx;
        assert_eq!(ctx.display, ctx2.display);
    }
}
