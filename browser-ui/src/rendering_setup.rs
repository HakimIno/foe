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
    // Cached published frame to avoid re-running BorrowedOpenGLTextureBuilder +
    // window.set_frame() on every paint when the underlying texture handle and
    // size haven't changed. Slint property updates have non-trivial overhead, so
    // skipping them when the frame is bit-identical to the last one matters.
    published_texture_id: u32,
    redraw_pending: bool,
    // Cached Slint GL state, captured on the first BeforeRendering tick.
    // Slint sets up the same context state every frame, so we avoid the
    // per-frame glGetIntegerv/glIsEnabled sync queries by querying once.
    prev_state_cached: bool,
    prev_active_texture: u32,
    prev_tex_2d: u32,
    prev_tex_rect: u32,
    prev_read_fbo: u32,
    prev_draw_fbo: u32,
    prev_scissor_enabled: bool,
    // Cached CGL context pointer. CGLGetCurrentContext is a TLS lookup —
    // not free, and unchanged for the lifetime of the Slint render thread.
    // We capture it the first time we observe a non-null context and reuse
    // it forever after, eliminating one syscall per frame.
    #[cfg(target_os = "macos")]
    cached_cgl_ctx: cgl::CGLContextObj,
    // FBO texture attachments persist on the FBO until reassigned. We only
    // need to call FramebufferTexture2D when the texture IDs change, which
    // they don't after RenderingSetup pre-allocates them. Tracking the last
    // attached IDs lets us skip the redundant attach calls each frame.
    read_fbo_attached_tex: u32,
    draw_fbo_attached_tex: u32,
}

thread_local! {
    static PUMP_TRIGGER: RefCell<Option<Box<dyn Fn()>>> = RefCell::new(None);
    static PAINT_TRIGGER: RefCell<Option<Box<dyn Fn() -> bool>>> = RefCell::new(None);

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

/// Trigger an on-demand paint of the active tab. Must be called from the main thread.
/// Returns `true` if an actual paint happened (used by the heartbeat to decide
/// whether to enter idle backoff). Returns immediately when no dirty flag is set.
pub fn trigger_paint() -> bool {
    PAINT_TRIGGER.with(|trigger| {
        if let Some(ref f) = *trigger.borrow() {
            f()
        } else {
            false
        }
    })
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

/// Setup rendering callbacks, the event-driven paint trigger, and a low-cost
/// heartbeat timer for the Servo engine.
///
/// Paint is primarily driven by `notify_new_frame_ready` (see delegate.rs), but a
/// 16ms heartbeat timer is also kept as a safety net: not every Servo internal
/// event (notably input event delivery) reliably fires SlintWaker.wake(), so input
/// responsiveness depended on the old polling pump. The heartbeat ticks are nearly
/// free when idle — `trigger_pump` short-circuits when Servo's queue is empty and
/// `trigger_paint` exits immediately when the dirty flag is unset.
///
/// Returns the timer; caller must keep it alive for the duration of the app.
pub fn setup_rendering(
    window: &AppWindow,
    servo_engine: Rc<RefCell<ServoEngine>>,
) -> slint::Timer {
    use slint::BorrowedOpenGLTextureBuilder;
    use slint::BorrowedOpenGLTextureOrigin;
    use slint::RenderingState;

    // GPU bridge (IOSurface zero-copy) is the default on macOS — the CPU
    // read-back path is kept only as an escape hatch via `FOE_DISABLE_GPU_BRIDGE=1`
    // because it forces a full GPU stall + multiple full-frame memcpys per frame.
    #[cfg(target_os = "macos")]
    let use_gpu_bridge = std::env::var_os("FOE_DISABLE_GPU_BRIDGE").is_none();

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
        published_texture_id: 0,
        redraw_pending: false,
        prev_state_cached: false,
        prev_active_texture: 0,
        prev_tex_2d: 0,
        prev_tex_rect: 0,
        prev_read_fbo: 0,
        prev_draw_fbo: 0,
        prev_scissor_enabled: false,
        #[cfg(target_os = "macos")]
        cached_cgl_ctx: std::ptr::null_mut(),
        read_fbo_attached_tex: 0,
        draw_fbo_attached_tex: 0,
    }));

    #[cfg(target_os = "macos")]
    {
        let shared_state_clone = shared_state.clone();
        let engine_for_notifier = servo_engine.clone();
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
                    // Slint is about to render — any redraw request we made has
                    // been picked up, so the next paint_trigger is allowed to
                    // queue a fresh one.
                    state.redraw_pending = false;

                    // Refresh the IOSurface pointer + size from the currently
                    // active tab's rendering context. This closes a use-after-
                    // free window: gpu_context::resize() frees the old IOSurface
                    // and allocates a new one, but state.iosurface only gets
                    // updated when paint_trigger runs. If Slint renders between
                    // those two events, we'd otherwise blit from a freed surface
                    // and see garbage (or a checker pattern of stale tiles).
                    if let Ok(engine) = engine_for_notifier.try_borrow() {
                        if let Some(tab) = engine.get_active_tab() {
                            if let Some(rc) = tab.rendering_context.as_ref() {
                                let live_iosurface = rc.get_iosurface();
                                let live_size = rc.size.get();
                                let live_psize = slint::PhysicalSize::new(
                                    live_size.width,
                                    live_size.height,
                                );
                                if state.iosurface != live_iosurface || state.size != live_psize {
                                    state.iosurface = live_iosurface;
                                    state.size = live_psize;
                                    // Force the rebind branch below to run since
                                    // the old binding now references freed memory.
                                    state.bound_iosurface = None;
                                    state.needs_bind = true;
                                }
                            }
                        }
                    }

                    if state.needs_bind && state.size.width > 0 && state.size.height > 0 {
                        let Some(iosurface) = state.iosurface else {
                            state.needs_bind = false;
                            return;
                        };

                        unsafe {
                            // CGLGetCurrentContext is a TLS lookup. The
                            // context is stable for the lifetime of the
                            // Slint render thread, so we capture it once
                            // and reuse — saves one syscall per frame.
                            let ctx = if !state.cached_cgl_ctx.is_null() {
                                state.cached_cgl_ctx
                            } else {
                                let c = cgl::CGLGetCurrentContext();
                                if !c.is_null() {
                                    state.cached_cgl_ctx = c;
                                }
                                c
                            };
                            if !ctx.is_null() {
                                // 1. Capture Slint's GL state once on the first frame; on
                                //    subsequent frames we trust the cached values, which avoids
                                //    five glGetIntegerv calls + one glIsEnabled per frame
                                //    (each a potential driver sync point).
                                if !state.prev_state_cached {
                                    let mut v = 0;
                                    gl::GetIntegerv(gl::ACTIVE_TEXTURE, &mut v);
                                    state.prev_active_texture = v as u32;

                                    let mut v = 0;
                                    gl::GetIntegerv(gl::TEXTURE_BINDING_2D, &mut v);
                                    state.prev_tex_2d = v as u32;

                                    let mut v = 0;
                                    gl::GetIntegerv(0x84F6, &mut v); // GL_TEXTURE_BINDING_RECTANGLE
                                    state.prev_tex_rect = v as u32;

                                    let mut v = 0;
                                    gl::GetIntegerv(gl::READ_FRAMEBUFFER_BINDING, &mut v);
                                    state.prev_read_fbo = v as u32;

                                    let mut v = 0;
                                    gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut v);
                                    state.prev_draw_fbo = v as u32;

                                    state.prev_scissor_enabled = gl::IsEnabled(gl::SCISSOR_TEST) != 0;

                                    state.prev_state_cached = true;
                                }

                                let scissor_enabled = state.prev_scissor_enabled;

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

                                // 6. Setup blit FBOs. FBO attachments are
                                // sticky in the GL spec — once attached,
                                // they stay until reassigned, so we only
                                // FramebufferTexture2D when the cached
                                // attachment doesn't match. After the
                                // first frame both are stable, dropping
                                // two GL calls per frame for free.
                                gl::BindFramebuffer(gl::READ_FRAMEBUFFER, state.read_fbo);
                                if state.read_fbo_attached_tex != state.texture_rect_id {
                                    gl::FramebufferTexture2D(
                                        gl::READ_FRAMEBUFFER,
                                        gl::COLOR_ATTACHMENT0,
                                        0x84F5, // GL_TEXTURE_RECTANGLE
                                        state.texture_rect_id,
                                        0,
                                    );
                                    state.read_fbo_attached_tex = state.texture_rect_id;
                                }

                                gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, state.draw_fbo);
                                if state.draw_fbo_attached_tex != state.texture_2d_id {
                                    gl::FramebufferTexture2D(
                                        gl::DRAW_FRAMEBUFFER,
                                        gl::COLOR_ATTACHMENT0,
                                        gl::TEXTURE_2D,
                                        state.texture_2d_id,
                                        0,
                                    );
                                    state.draw_fbo_attached_tex = state.texture_2d_id;
                                }

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

                                gl::BindFramebuffer(gl::READ_FRAMEBUFFER, state.prev_read_fbo);
                                gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, state.prev_draw_fbo);

                                gl::ActiveTexture(state.prev_active_texture);
                                gl::BindTexture(gl::TEXTURE_2D, state.prev_tex_2d);
                                gl::BindTexture(0x84F5, state.prev_tex_rect);

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
                    state.prev_state_cached = false;
                    #[cfg(target_os = "macos")]
                    {
                        state.cached_cgl_ctx = std::ptr::null_mut();
                    }
                    state.read_fbo_attached_tex = 0;
                    state.draw_fbo_attached_tex = 0;
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

    // Event-driven paint trigger. Invoked by the delegate when Servo signals a
    // new frame is ready; cheaply no-ops when the dirty flag isn't set.
    let engine_clone = servo_engine.clone();
    let window_weak = window.as_weak();

    #[cfg(target_os = "macos")]
    let shared_state_clone_for_paint = shared_state.clone();

    #[cfg(target_os = "macos")]
    let use_gpu_bridge_for_paint = use_gpu_bridge;

    let paint_trigger: Box<dyn Fn() -> bool> = Box::new(move || -> bool {
        let engine = engine_clone.borrow();

        if !engine.has_active_webview() {
            let _ = crate::servo_engine::take_active_dirty();
            return false;
        }

        if !crate::servo_engine::take_active_dirty() {
            return false;
        }

        let start = Instant::now();
        engine.paint_active();

        #[cfg(target_os = "macos")]
        {
            if use_gpu_bridge_for_paint {
                if let Some(tab) = engine.get_active_tab() {
                    let Some(rendering_context) = tab.rendering_context.as_ref() else {
                        return false;
                    };
                    let iosurface = rendering_context.get_iosurface();
                    let size = rendering_context.size.get();

                    let mut state = shared_state_clone_for_paint.borrow_mut();
                    state.iosurface = iosurface;
                    state.size = slint::PhysicalSize::new(size.width, size.height);
                    state.needs_bind = true;

                    // Publish a new slint::Image only when something Slint would
                    // care about actually changed (texture id, size, or the
                    // backing IOSurface). Otherwise just request a redraw so
                    // BeforeRendering re-blits the IOSurface into texture_2d.
                    let should_publish_frame = !state.frame_published
                        || state.published_size != state.size
                        || state.published_texture_id != state.texture_2d_id
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
                        state.published_texture_id = state.texture_2d_id;
                        // set_frame implicitly schedules a redraw, so mark as pending.
                        state.redraw_pending = true;
                    } else if !state.redraw_pending {
                        if let Some(window) = window_weak.upgrade() {
                            window.window().request_redraw();
                            state.redraw_pending = true;
                        }
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
        true
    });

    PAINT_TRIGGER.with(|t| {
        *t.borrow_mut() = Some(paint_trigger);
    });

    // Adaptive heartbeat: ticks at 16ms baseline so input + frame delivery feels
    // snappy (≈ 60Hz), but enters a "slow" mode when nothing has actually
    // painted for ~20 ticks (~320ms). In slow mode we still tick every 16ms but
    // only run pump/paint on every 8th tick (~128ms effective), cutting idle
    // CPU and JS-error-processing rate substantially. The first dirty frame
    // snaps us back to fast mode immediately.
    let heartbeat = slint::Timer::default();
    let mut idle_ticks: u32 = 0;
    let mut slow_skip: u32 = 0;
    const IDLE_THRESHOLD: u32 = 20; // ticks of no-paint before slow mode
    const SLOW_DIVISOR: u32 = 8;    // do real work 1 of every N ticks in slow mode
    heartbeat.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let in_slow_mode = idle_ticks >= IDLE_THRESHOLD;
            if in_slow_mode {
                slow_skip = slow_skip.saturating_add(1);
                if slow_skip < SLOW_DIVISOR {
                    return;
                }
                slow_skip = 0;
            }

            trigger_pump();
            let painted = trigger_paint();

            if painted {
                idle_ticks = 0;
                slow_skip = 0;
            } else {
                idle_ticks = idle_ticks.saturating_add(1);
            }
        },
    );

    heartbeat
}
