use crate::servo_engine::ServoEngine;
use crate::AppWindow;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

struct SharedState {
    #[allow(dead_code)]
    iosurface: Option<*const std::ffi::c_void>,
    bound_iosurface: Option<*const std::ffi::c_void>,
    texture_rect_id: u32,
    texture_2d_id: u32,
    read_fbo: u32,
    draw_fbo: u32,
    size: slint::PhysicalSize,
    allocated_size: slint::PhysicalSize,
    needs_bind: bool,
    frame_published: bool,
    published_size: slint::PhysicalSize,
}

thread_local! {
    static PUMP_TRIGGER: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);

    // Performance profiling metrics
    static FRAME_COUNT: RefCell<usize> = RefCell::new(0);
    static TOTAL_RENDER_TIME: RefCell<std::time::Duration> = RefCell::new(std::time::Duration::from_secs(0));
    static LAST_PROFILE_PRINT: RefCell<Instant> = RefCell::new(Instant::now());
}

/// Trigger the thread-local event loop pump callback. Must be called from the main thread.
pub fn trigger_pump() {
    PUMP_TRIGGER.with(|trigger| {
        if let Some(ref f) = *trigger.borrow() {
            f();
        }
    });
}

/// Profile and print the render metrics every second.
fn profile_frame(duration: std::time::Duration) {
    FRAME_COUNT.with(|fc| {
        *fc.borrow_mut() += 1;
    });
    TOTAL_RENDER_TIME.with(|trt| {
        *trt.borrow_mut() += duration;
    });

    LAST_PROFILE_PRINT.with(|lpp| {
        let now = Instant::now();
        let elapsed = now.duration_since(*lpp.borrow());
        if elapsed >= std::time::Duration::from_secs(1) {
            let frames = FRAME_COUNT.with(|fc| {
                let val = *fc.borrow();
                *fc.borrow_mut() = 0;
                val
            });
            let total_time = TOTAL_RENDER_TIME.with(|trt| {
                let val = *trt.borrow();
                *trt.borrow_mut() = std::time::Duration::from_secs(0);
                val
            });
            *lpp.borrow_mut() = now;

            let avg_time_ms = if frames > 0 {
                total_time.as_secs_f64() * 1000.0 / frames as f64
            } else {
                0.0
            };

            log::info!(
                "[Profile] FPS: {} | Avg Render Time: {:.2}ms",
                frames,
                avg_time_ms
            );
            println!(
                "[Performance Profile] FPS: {} | Avg Render Time: {:.2}ms 🚀",
                frames, avg_time_ms
            );
        }
    });
}

/// Setup rendering callbacks and the paint timer loop for the Servo engine.
/// Returns the timer that must be kept alive for the duration of the application.
pub fn setup_rendering(window: &AppWindow, servo_engine: Rc<RefCell<ServoEngine>>) -> slint::Timer {
    use slint::BorrowedOpenGLTextureBuilder;
    use slint::BorrowedOpenGLTextureOrigin;
    use slint::RenderingState;

    #[cfg(target_os = "macos")]
    let use_gpu_bridge = std::env::var_os("FOE_USE_GPU_BRIDGE").is_some();

    let shared_state = Rc::new(RefCell::new(SharedState {
        iosurface: None,
        bound_iosurface: None,
        texture_rect_id: 0,
        texture_2d_id: 0,
        read_fbo: 0,
        draw_fbo: 0,
        size: slint::PhysicalSize::new(0, 0),
        allocated_size: slint::PhysicalSize::new(0, 0),
        needs_bind: false,
        frame_published: false,
        published_size: slint::PhysicalSize::new(0, 0),
    }));

    #[cfg(target_os = "macos")]
    {
        let shared_state_clone = shared_state.clone();
        let _ = window.window().set_rendering_notifier(move |state, graphics_api| {
            match state {
                RenderingState::RenderingSetup => {
                    if let slint::GraphicsAPI::NativeOpenGL { get_proc_address } = graphics_api {
                        gl::load_with(|s| {
                            let cstr = std::ffi::CString::new(s).unwrap();
                            get_proc_address(&cstr)
                        });

                        // Pre-allocate texture and frame buffer IDs immediately during setup
                        // so they are valid and non-zero when the first frame is built
                        let mut state = shared_state_clone.borrow_mut();
                        unsafe {
                            if state.texture_rect_id == 0 {
                                let mut tex = 0;
                                gl::GenTextures(1, &mut tex);
                                state.texture_rect_id = tex;
                            }
                            if state.texture_2d_id == 0 {
                                let mut tex = 0;
                                gl::GenTextures(1, &mut tex);
                                state.texture_2d_id = tex;
                            }
                            if state.read_fbo == 0 {
                                let mut fbo = 0;
                                gl::GenFramebuffers(1, &mut fbo);
                                state.read_fbo = fbo;
                            }
                            if state.draw_fbo == 0 {
                                let mut fbo = 0;
                                gl::GenFramebuffers(1, &mut fbo);
                                state.draw_fbo = fbo;
                            }
                        }
                    }
                }
                RenderingState::BeforeRendering => {
                    let mut state = shared_state_clone.borrow_mut();
                    if state.needs_bind && state.size.width > 0 && state.size.height > 0 {
                        let Some(iosurface) = state.iosurface else {
                            state.needs_bind = false;
                            return;
                        };

                        unsafe {
                            let ctx = cgl::CGLGetCurrentContext();
                            if !ctx.is_null() {
                                // 1. Save all previous OpenGL states to avoid corrupting Slint's state
                                let mut prev_active_texture = 0;
                                gl::GetIntegerv(gl::ACTIVE_TEXTURE, &mut prev_active_texture);

                                let mut prev_tex_2d = 0;
                                gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut prev_tex_2d);

                                let mut prev_tex_rect = 0;
                                gl::GetIntegerv(0x84F6, &mut prev_tex_rect); // GL_TEXTURE_BINDING_RECTANGLE

                                let mut prev_read_fbo = 0;
                                let mut prev_draw_fbo = 0;
                                gl::GetIntegerv(gl::READ_FRAMEBUFFER_BINDING, &mut prev_read_fbo);
                                gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut prev_draw_fbo);

                                let scissor_enabled = gl::IsEnabled(gl::SCISSOR_TEST) != 0;

                                // 2. Ensure texture IDs and FBOs are created (fallback just in case)
                                if state.texture_rect_id == 0 {
                                    let mut tex = 0;
                                    gl::GenTextures(1, &mut tex);
                                    state.texture_rect_id = tex;
                                }
                                if state.texture_2d_id == 0 {
                                    let mut tex = 0;
                                    gl::GenTextures(1, &mut tex);
                                    state.texture_2d_id = tex;
                                }
                                if state.read_fbo == 0 {
                                    let mut fbo = 0;
                                    gl::GenFramebuffers(1, &mut fbo);
                                    state.read_fbo = fbo;
                                }
                                if state.draw_fbo == 0 {
                                    let mut fbo = 0;
                                    gl::GenFramebuffers(1, &mut fbo);
                                    state.draw_fbo = fbo;
                                }

                                // 3. Bind IOSurface to texture_rect_id (GL_TEXTURE_RECTANGLE = 0x84F5)
                                // We MUST call gl::BindTexture every frame to trigger driver-level sync of IOSurface changes.
                                gl::BindTexture(0x84F5, state.texture_rect_id);

                                // Only call CGLTexImageIOSurface2D if the surface pointer has changed
                                let iosurface_changed = state.bound_iosurface != Some(iosurface);
                                if iosurface_changed {
                                    gl::TexParameteri(0x84F5, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
                                    gl::TexParameteri(0x84F5, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);

                                    let err = crate::servo_engine::gpu_context::macos_iosurface::CGLTexImageIOSurface2D(
                                        ctx,
                                        0x84F5, // GL_TEXTURE_RECTANGLE
                                        gl::RGBA as u32,
                                        state.size.width as i32,
                                        state.size.height as i32,
                                        gl::BGRA,
                                        0x8367, // GL_UNSIGNED_INT_8_8_8_8_REV
                                        iosurface,
                                        0,
                                    );

                                    if err != 0 {
                                        log::error!("[Slint Render] CGLTexImageIOSurface2D failed with error {}", err);
                                    } else {
                                        state.bound_iosurface = Some(iosurface);
                                    }
                                }

                                // 4. Initialize texture_2d_id (GL_TEXTURE_2D) only if size changed
                                if state.allocated_size != state.size {
                                    gl::BindTexture(gl::TEXTURE_2D, state.texture_2d_id);
                                    gl::TexImage2D(
                                        gl::TEXTURE_2D,
                                        0,
                                        gl::RGBA as i32,
                                        state.size.width as i32,
                                        state.size.height as i32,
                                        0,
                                        gl::RGBA,
                                        gl::UNSIGNED_BYTE,
                                        std::ptr::null(),
                                    );
                                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
                                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
                                    state.allocated_size = state.size;
                                }

                                // 5. Disable scissor test temporarily to avoid clipping the blit operation
                                if scissor_enabled {
                                    gl::Disable(gl::SCISSOR_TEST);
                                }

                                // 6. Setup blit FBOs
                                gl::BindFramebuffer(gl::READ_FRAMEBUFFER, state.read_fbo);
                                gl::FramebufferTexture2D(
                                    gl::READ_FRAMEBUFFER,
                                    gl::COLOR_ATTACHMENT0,
                                    0x84F5, // GL_TEXTURE_RECTANGLE
                                    state.texture_rect_id,
                                    0,
                                );

                                gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, state.draw_fbo);
                                gl::FramebufferTexture2D(
                                    gl::DRAW_FRAMEBUFFER,
                                    gl::COLOR_ATTACHMENT0,
                                    gl::TEXTURE_2D,
                                    state.texture_2d_id,
                                    0,
                                );

                                // 7. Blit!
                                gl::BlitFramebuffer(
                                    0, 0, state.size.width as i32, state.size.height as i32,
                                    0, 0, state.size.width as i32, state.size.height as i32,
                                    gl::COLOR_BUFFER_BIT,
                                    gl::NEAREST,
                                );

                                // 8. Restore previous OpenGL states in reverse order
                                if scissor_enabled {
                                    gl::Enable(gl::SCISSOR_TEST);
                                }

                                gl::BindFramebuffer(gl::READ_FRAMEBUFFER, prev_read_fbo as u32);
                                gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, prev_draw_fbo as u32);

                                gl::ActiveTexture(prev_active_texture as u32);
                                gl::BindTexture(gl::TEXTURE_2D, prev_tex_2d as u32);
                                gl::BindTexture(0x84F5, prev_tex_rect as u32);

                                log::debug!("[Slint Render] Successfully blitted IOSurface to GL_TEXTURE_2D");
                            } else {
                                log::error!("[Slint Render] Current CGL context is null!");
                            }
                        }
                        state.needs_bind = false;
                    }
                }
                RenderingState::RenderingTeardown => {
                    let mut state = shared_state_clone.borrow_mut();
                    unsafe {
                        if state.texture_rect_id != 0 {
                            gl::DeleteTextures(1, &state.texture_rect_id);
                            state.texture_rect_id = 0;
                        }
                        if state.texture_2d_id != 0 {
                            gl::DeleteTextures(1, &state.texture_2d_id);
                            state.texture_2d_id = 0;
                        }
                        if state.read_fbo != 0 {
                            gl::DeleteFramebuffers(1, &state.read_fbo);
                            state.read_fbo = 0;
                        }
                        if state.draw_fbo != 0 {
                            gl::DeleteFramebuffers(1, &state.draw_fbo);
                            state.draw_fbo = 0;
                        }
                    }
                    state.bound_iosurface = None;
                    state.frame_published = false;
                    state.published_size = slint::PhysicalSize::new(0, 0);
                }
                _ => {}
            }
        });
    }

    // Register main-thread waker event pump callback
    let engine_clone_for_pump = servo_engine.clone();
    let pump_trigger = Box::new(move || {
        let engine = engine_clone_for_pump.borrow();
        engine.spin_event_loop();
    });

    PUMP_TRIGGER.with(|t| {
        *t.borrow_mut() = Some(pump_trigger);
    });

    // Event pump & paint timer (runs at 8ms intervals to match 120Hz monitors, throttling paint rate)
    let engine_clone = servo_engine.clone();
    let window_weak = window.as_weak();
    let servo_timer = slint::Timer::default();

    #[cfg(target_os = "macos")]
    let shared_state_clone_for_timer = shared_state.clone();

    #[cfg(target_os = "macos")]
    let use_gpu_bridge_for_timer = use_gpu_bridge;

    servo_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let engine = engine_clone.borrow();

            // 1. Pump the Servo event loop to process inputs, layout, and compositing tasks
            engine.spin_event_loop();

            if !engine.has_active_webview() {
                let _ = crate::servo_engine::take_active_dirty();
                return;
            }

            // 2. Throttled Paint: Only paint and update frame if the dirty flag is set
            if crate::servo_engine::take_active_dirty() {
                let start = Instant::now();
                engine.paint_active();

                #[cfg(target_os = "macos")]
                {
                    if use_gpu_bridge_for_timer {
                        if let Some(tab) = engine.get_active_tab() {
                            let Some(rendering_context) = tab.rendering_context.as_ref() else {
                                return;
                            };
                            let iosurface = rendering_context.get_iosurface();
                            let size = rendering_context.size.get();

                            let mut state = shared_state_clone_for_timer.borrow_mut();
                            state.iosurface = iosurface;
                            state.size = slint::PhysicalSize::new(size.width, size.height);
                            state.needs_bind = true;

                            let should_publish_frame = !state.frame_published
                                || state.published_size != state.size
                                || state.bound_iosurface != iosurface;

                            if should_publish_frame
                                && iosurface.is_some()
                                && state.texture_2d_id != 0
                                && size.width > 0
                                && size.height > 0
                            {
                                let frame = unsafe {
                                    BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                                        core::num::NonZeroU32::new(state.texture_2d_id).unwrap(),
                                        (size.width, size.height).into(),
                                    )
                                }
                                .origin(BorrowedOpenGLTextureOrigin::BottomLeft)
                                .build();

                                if let Some(window) = window_weak.upgrade() {
                                    window.set_frame(frame);
                                }
                                state.frame_published = true;
                                state.published_size = state.size;
                            } else if let Some(window) = window_weak.upgrade() {
                                window.window().request_redraw();
                            }
                        }
                    } else if let Some(frame) = engine.get_active_frame() {
                        if let Some(window) = window_weak.upgrade() {
                            window.set_frame(frame);
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    if let Some(frame) = engine.get_active_frame() {
                        if let Some(window) = window_weak.upgrade() {
                            window.set_frame(frame);
                        }
                    }
                }

                profile_frame(start.elapsed());
            }
        },
    );

    servo_timer
}
