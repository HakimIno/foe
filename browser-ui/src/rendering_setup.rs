use crate::AppWindow;
use crate::servo_engine::ServoEngine;
use std::rc::Rc;
use std::cell::RefCell;
use slint::ComponentHandle;

struct SharedState {
    #[allow(dead_code)]
    iosurface: Option<*const std::ffi::c_void>,
    texture_rect_id: u32,
    texture_2d_id: u32,
    read_fbo: u32,
    draw_fbo: u32,
    size: slint::PhysicalSize,
    allocated_size: slint::PhysicalSize,
    needs_bind: bool,
}

/// Setup rendering callbacks and the paint timer loop for the Servo engine.
/// Returns the timer that must be kept alive for the duration of the application.
pub fn setup_rendering(
    window: &AppWindow,
    servo_engine: Rc<RefCell<ServoEngine>>,
) -> slint::Timer {
    use slint::BorrowedOpenGLTextureBuilder;
    use slint::BorrowedOpenGLTextureOrigin;
    use slint::RenderingState;

    let shared_state = Rc::new(RefCell::new(SharedState {
        iosurface: None,
        texture_rect_id: 0,
        texture_2d_id: 0,
        read_fbo: 0,
        draw_fbo: 0,
        size: slint::PhysicalSize::new(0, 0),
        allocated_size: slint::PhysicalSize::new(0, 0),
        needs_bind: false,
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
                    }
                }
                RenderingState::BeforeRendering => {
                    let mut state = shared_state_clone.borrow_mut();
                    if state.needs_bind {
                        if let Some(iosurface) = state.iosurface {
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

                                    // 2. Ensure texture IDs and FBOs are created
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
                                    gl::BindTexture(0x84F5, state.texture_rect_id);
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
                }
                _ => {}
            }
        });
    }

    let engine_clone = servo_engine.clone();
    let window_weak = window.as_weak();
    let servo_timer = slint::Timer::default();

    #[cfg(target_os = "macos")]
    let shared_state_clone_for_timer = shared_state.clone();

    servo_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let engine = engine_clone.borrow();
            engine.spin_event_loop();

            if crate::servo_engine::take_active_dirty() {
                engine.paint_active();

                #[cfg(target_os = "macos")]
                {
                    if let Some(tab) = engine.get_active_tab() {
                        let iosurface = tab.rendering_context.get_iosurface();
                        let size = tab.rendering_context.size.get();

                        let mut state = shared_state_clone_for_timer.borrow_mut();
                        state.iosurface = iosurface;
                        state.size = slint::PhysicalSize::new(size.width, size.height);
                        state.needs_bind = true;

                        if state.texture_2d_id != 0 && size.width > 0 && size.height > 0 {
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
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    match engine.get_active_frame() {
                        Some(frame) => {
                            let (w, h) = (frame.size().width, frame.size().height);
                            log::debug!("[Render] Got frame {}x{}", w, h);
                            if let Some(window) = window_weak.upgrade() {
                                window.set_frame(frame);
                            }
                        }
                        None => {
                            log::warn!("[Render] get_active_frame returned None");
                        }
                    }
                }
            }
        },
    );

    servo_timer
}
