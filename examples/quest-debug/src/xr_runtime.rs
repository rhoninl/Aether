//! Real OpenXR runtime for Quest 3.
//!
//! Uses the `openxr` crate to create an actual session, swapchains,
//! and submit frames to the Quest display.

use openxr as xr;

use crate::egl::EglContext;

const VIEW_COUNT: u32 = 2; // stereo

/// GLES function pointers for rendering.
struct GlFns {
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    clear: unsafe extern "C" fn(u32),
    bind_framebuffer: unsafe extern "C" fn(u32, u32),
    framebuffer_texture_2d: unsafe extern "C" fn(u32, u32, u32, u32, i32),
    gen_framebuffers: unsafe extern "C" fn(i32, *mut u32),
    viewport: unsafe extern "C" fn(i32, i32, i32, i32),
}

const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_TEXTURE_2D: u32 = 0x0DE1;

/// Run the real OpenXR render loop on Quest.
pub fn run_xr_loop(egl: &EglContext) -> Result<(), String> {
    // Load OpenXR runtime.
    // On Quest 3, libopenxr_forwardloader.so has lazy symbol resolution —
    // the openxr crate's Entry::load_from uses RTLD_NOW which fails.
    // We manually dlopen with RTLD_LAZY and extract xrGetInstanceProcAddr.
    let entry = unsafe { load_openxr_entry()? };

    // Create instance with GLES extension
    let app_info = xr::ApplicationInfo {
        application_name: "Aether Quest Debug",
        application_version: 1,
        engine_name: "Aether Engine",
        engine_version: 1,
        api_version: xr::Version::new(1, 0, 0),
    };

    let extensions = entry
        .enumerate_extensions()
        .map_err(|e| format!("enumerate extensions: {e}"))?;
    log::info!(
        "OpenXR extensions: GLES={}",
        extensions.khr_opengl_es_enable
    );

    let mut enabled_extensions = xr::ExtensionSet::default();
    enabled_extensions.khr_opengl_es_enable = true;

    let instance = entry
        .create_instance(&app_info, &enabled_extensions, &[])
        .map_err(|e| format!("create instance: {e}"))?;

    let instance_props = instance
        .properties()
        .map_err(|e| format!("instance props: {e}"))?;
    log::info!(
        "OpenXR runtime: {} v{}",
        instance_props.runtime_name,
        instance_props.runtime_version
    );

    // Get HMD system
    let system = instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .map_err(|e| format!("get system: {e}"))?;

    // Check GLES requirements
    let _reqs = instance
        .graphics_requirements::<xr::OpenGlEs>(system)
        .map_err(|e| format!("graphics requirements: {e}"))?;

    // Create session with EGL binding
    let session_create_info = xr::opengles::SessionCreateInfo::Android {
        display: egl.display as *mut _,
        config: egl.config as *mut _,
        context: egl.context as *mut _,
    };

    let (session, mut frame_waiter, mut frame_stream) = unsafe {
        instance
            .create_session::<xr::OpenGlEs>(system, &session_create_info)
            .map_err(|e| format!("create session: {e}"))?
    };

    // Create reference space (stage = floor level)
    let stage = session
        .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)
        .map_err(|e| format!("create space: {e}"))?;

    // Get view configuration
    let views = instance
        .enumerate_view_configuration_views(system, xr::ViewConfigurationType::PRIMARY_STEREO)
        .map_err(|e| format!("enumerate views: {e}"))?;
    log::info!(
        "View config: {}x{} per eye",
        views[0].recommended_image_rect_width,
        views[0].recommended_image_rect_height
    );

    let eye_width = views[0].recommended_image_rect_width;
    let eye_height = views[0].recommended_image_rect_height;

    // Enumerate swapchain formats and pick sRGB
    let formats = session
        .enumerate_swapchain_formats()
        .map_err(|e| format!("enumerate formats: {e}"))?;
    // GL_SRGB8_ALPHA8 = 0x8C43, GL_RGBA8 = 0x8058
    let format = if formats.contains(&0x8C43) {
        0x8C43_u32 // GL_SRGB8_ALPHA8
    } else if formats.contains(&0x8058) {
        0x8058_u32 // GL_RGBA8
    } else {
        formats[0] as u32
    };

    // Create one swapchain per eye
    let mut swapchains = Vec::new();
    let mut swapchain_images = Vec::new();
    for _ in 0..VIEW_COUNT {
        let swapchain = session
            .create_swapchain(&xr::SwapchainCreateInfo {
                create_flags: xr::SwapchainCreateFlags::EMPTY,
                usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                    | xr::SwapchainUsageFlags::SAMPLED,
                format,
                sample_count: 1,
                width: eye_width,
                height: eye_height,
                face_count: 1,
                array_size: 1,
                mip_count: 1,
            })
            .map_err(|e| format!("create swapchain: {e}"))?;

        let images = swapchain
            .enumerate_images()
            .map_err(|e| format!("enumerate images: {e}"))?;
        swapchain_images.push(images);
        swapchains.push(swapchain);
    }

    // Load GL functions
    let gl = load_gl_fns()?;
    let mut fbo: u32 = 0;
    unsafe {
        (gl.gen_framebuffers)(1, &mut fbo);
    }

    log::info!("OpenXR ready, entering render loop");

    // Begin session
    session
        .begin(xr::ViewConfigurationType::PRIMARY_STEREO)
        .map_err(|e| format!("begin session: {e}"))?;

    let mut running = true;
    let mut frame_count: u64 = 0;

    while running {
        // Poll events
        let mut buf = xr::EventDataBuffer::new();
        while let Some(event) = instance
            .poll_event(&mut buf)
            .map_err(|e| format!("poll: {e}"))?
        {
            match event {
                xr::Event::SessionStateChanged(ev) => {
                    log::info!("Session state: {:?}", ev.state());
                    match ev.state() {
                        xr::SessionState::STOPPING => {
                            session.end().map_err(|e| format!("end session: {e}"))?;
                            running = false;
                        }
                        xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                            running = false;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if !running {
            break;
        }

        // Wait for frame
        let frame_state = frame_waiter
            .wait()
            .map_err(|e| format!("wait frame: {e}"))?;

        frame_stream
            .begin()
            .map_err(|e| format!("begin frame: {e}"))?;

        if frame_state.should_render {
            // Locate views (eye poses)
            let (_, view_states) = session
                .locate_views(
                    xr::ViewConfigurationType::PRIMARY_STEREO,
                    frame_state.predicted_display_time,
                    &stage,
                )
                .map_err(|e| format!("locate views: {e}"))?;

            // Phase 1: Render each eye (requires mutable swapchain access)
            let mut eye_views: Vec<(xr::Posef, xr::Fovf)> = Vec::new();
            for (i, view) in view_states.iter().enumerate() {
                let swapchain = &mut swapchains[i];
                let image_index = swapchain
                    .acquire_image()
                    .map_err(|e| format!("acquire: {e}"))?;

                swapchain
                    .wait_image(xr::Duration::from_nanos(100_000_000))
                    .map_err(|e| format!("wait image: {e}"))?;

                let gl_image = swapchain_images[i][image_index as usize];

                unsafe {
                    (gl.bind_framebuffer)(GL_FRAMEBUFFER, fbo);
                    (gl.framebuffer_texture_2d)(
                        GL_FRAMEBUFFER,
                        GL_COLOR_ATTACHMENT0,
                        GL_TEXTURE_2D,
                        gl_image,
                        0,
                    );
                    (gl.viewport)(0, 0, eye_width as i32, eye_height as i32);

                    let t = (frame_count as f32 * 0.01).sin() * 0.5 + 0.5;
                    (gl.clear_color)(0.05, 0.05 + t * 0.1, 0.15 + t * 0.1, 1.0);
                    (gl.clear)(GL_COLOR_BUFFER_BIT);
                }

                swapchain
                    .release_image()
                    .map_err(|e| format!("release: {e}"))?;

                eye_views.push((view.pose, view.fov));
            }

            // Phase 2: Build projection views (immutable swapchain refs)
            let rect = xr::Rect2Di {
                offset: xr::Offset2Di { x: 0, y: 0 },
                extent: xr::Extent2Di {
                    width: eye_width as i32,
                    height: eye_height as i32,
                },
            };

            let projection_views: Vec<_> = eye_views
                .iter()
                .enumerate()
                .map(|(i, (pose, fov))| {
                    xr::CompositionLayerProjectionView::new()
                        .pose(*pose)
                        .fov(*fov)
                        .sub_image(
                            xr::SwapchainSubImage::new()
                                .swapchain(&swapchains[i])
                                .image_rect(rect),
                        )
                })
                .collect();

            let projection_layer = xr::CompositionLayerProjection::new()
                .space(&stage)
                .views(&projection_views);

            frame_stream
                .end(
                    frame_state.predicted_display_time,
                    xr::EnvironmentBlendMode::OPAQUE,
                    &[&projection_layer],
                )
                .map_err(|e| format!("end frame: {e}"))?;
        } else {
            frame_stream
                .end(
                    frame_state.predicted_display_time,
                    xr::EnvironmentBlendMode::OPAQUE,
                    &[],
                )
                .map_err(|e| format!("end frame: {e}"))?;
        }

        frame_count += 1;
        if frame_count % 500 == 0 {
            log::info!("Frame {frame_count}");
        }
    }

    log::info!("XR loop ended after {frame_count} frames");
    Ok(())
}

fn load_gl_fns() -> Result<GlFns, String> {
    unsafe {
        let lib = dlopen(b"libGLESv3.so\0".as_ptr() as _, 1);
        if lib.is_null() {
            let lib2 = dlopen(b"libGLESv2.so\0".as_ptr() as _, 1);
            if lib2.is_null() {
                return Err("failed to load libGLESv3.so or libGLESv2.so".to_string());
            }
            return load_gl_from(lib2);
        }
        load_gl_from(lib)
    }
}

unsafe fn load_gl_from(lib: *mut std::ffi::c_void) -> Result<GlFns, String> {
    Ok(GlFns {
        clear_color: load_sym(lib, b"glClearColor\0")?,
        clear: load_sym(lib, b"glClear\0")?,
        bind_framebuffer: load_sym(lib, b"glBindFramebuffer\0")?,
        framebuffer_texture_2d: load_sym(lib, b"glFramebufferTexture2D\0")?,
        gen_framebuffers: load_sym(lib, b"glGenFramebuffers\0")?,
        viewport: load_sym(lib, b"glViewport\0")?,
    })
}

unsafe fn load_sym<T>(lib: *mut std::ffi::c_void, name: &[u8]) -> Result<T, String> {
    let sym = dlsym(lib, name.as_ptr() as _);
    if sym.is_null() {
        return Err(format!(
            "GL: {}",
            std::str::from_utf8(&name[..name.len() - 1]).unwrap_or("?")
        ));
    }
    Ok(std::mem::transmute_copy(&sym))
}

const RTLD_LAZY: i32 = 0x0001;

/// Load OpenXR entry using RTLD_LAZY to handle the Quest forward loader's
/// unresolved symbols (they're resolved at runtime by the XR broker).
unsafe fn load_openxr_entry() -> Result<xr::Entry, String> {
    let loader_names = ["libopenxr_forwardloader.so", "libopenxr_loader.so"];

    for name in &loader_names {
        log::info!("Trying OpenXR loader: {name} (RTLD_LAZY)");
        let c_name = std::ffi::CString::new(*name).unwrap();
        let lib = dlopen(c_name.as_ptr(), RTLD_LAZY);
        if lib.is_null() {
            log::warn!("dlopen failed for {name}");
            continue;
        }

        let sym = dlsym(lib, b"xrGetInstanceProcAddr\0".as_ptr() as _);
        if sym.is_null() {
            log::warn!("{name}: xrGetInstanceProcAddr not found");
            continue;
        }

        log::info!("OpenXR loaded from {name}");
        let get_instance_proc_addr: openxr_sys::pfn::GetInstanceProcAddr = std::mem::transmute(sym);
        return xr::Entry::from_get_instance_proc_addr(get_instance_proc_addr)
            .map_err(|e| format!("OpenXR entry init: {e}"));
    }

    Err(
        "OpenXR: no loader found (tried libopenxr_forwardloader.so, libopenxr_loader.so)"
            .to_string(),
    )
}

extern "C" {
    fn dlopen(filename: *const std::ffi::c_char, flags: i32) -> *mut std::ffi::c_void;
    fn dlsym(
        handle: *mut std::ffi::c_void,
        symbol: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void;
}
