use servo::{RenderingContext, DeviceIntRect};
use std::rc::Rc;
use std::sync::Arc;
use std::cell::{Cell, RefCell};
use surfman::{Connection, Adapter, Device, Context, Surface, SurfaceTexture};
use dpi::PhysicalSize;
use image::RgbaImage;
use gleam::gl::{self, Gl};

/// Custom texture-backed FBO that WebRender renders into.
/// Mirrors the `Framebuffer` struct in servo-paint-api's OffscreenRenderingContext.
struct CustomFramebuffer {
    gl: Rc<dyn Gl>,
    framebuffer_id: u32,
    texture_id: u32,
    renderbuffer_id: u32,
}

impl CustomFramebuffer {
    fn new(gl: Rc<dyn Gl>, size: PhysicalSize<u32>) -> Self {
        let fbo = gl.gen_framebuffers(1)[0];
        gl.bind_framebuffer(gl::FRAMEBUFFER, fbo);

        let tex = gl.gen_textures(1)[0];
        gl.bind_texture(gl::TEXTURE_2D, tex);
        gl.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as gl::GLint,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            None,
        );
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as gl::GLint);
        gl.tex_parameter_i(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as gl::GLint);
        gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, tex, 0);
        gl.bind_texture(gl::TEXTURE_2D, 0);

        let rbo = gl.gen_renderbuffers(1)[0];
        gl.bind_renderbuffer(gl::RENDERBUFFER, rbo);
        gl.renderbuffer_storage(
            gl::RENDERBUFFER,
            gl::DEPTH_COMPONENT24,
            size.width as gl::GLsizei,
            size.height as gl::GLsizei,
        );
        gl.framebuffer_renderbuffer(gl::FRAMEBUFFER, gl::DEPTH_ATTACHMENT, gl::RENDERBUFFER, rbo);

        let status = gl.check_frame_buffer_status(gl::FRAMEBUFFER);
        if status != gl::FRAMEBUFFER_COMPLETE {
            log::error!("[GpuCtx] FBO {} not complete: 0x{:x}", fbo, status);
        } else {
            log::debug!("[GpuCtx] Created FBO {} ({}x{})", fbo, size.width, size.height);
        }

        Self { gl, framebuffer_id: fbo, texture_id: tex, renderbuffer_id: rbo }
    }

    fn bind(&self) {
        self.gl.bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id);
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        self.gl.bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id);
        self.gl.bind_vertex_array(0);
        self.gl.finish();

        let x = source_rectangle.min.x;
        let y = source_rectangle.min.y;
        let w = source_rectangle.width();
        let h = source_rectangle.height();

        let pixels = self.gl.read_pixels(x, y, w, h, gl::RGBA, gl::UNSIGNED_BYTE);

        let gl_err = self.gl.get_error();
        if gl_err != gl::NO_ERROR {
            log::warn!("[GpuCtx] GL error 0x{:x} after read_pixels (fbo={})", gl_err, self.framebuffer_id);
        }

        if log::log_enabled!(log::Level::Debug) && pixels.len() >= 16 {
            let cx = w as usize / 2;
            let cy = h as usize / 2;
            let ci = (cy * w as usize + cx) * 4;
            let cp = if ci + 3 < pixels.len() {
                [pixels[ci], pixels[ci+1], pixels[ci+2], pixels[ci+3]]
            } else {
                [0, 0, 0, 0]
            };
            log::debug!(
                "[GpuCtx] {}x{} corner=[{},{},{},{}] center=[{},{},{},{}]",
                w, h,
                pixels[0], pixels[1], pixels[2], pixels[3],
                cp[0], cp[1], cp[2], cp[3]
            );
        }

        // Flip vertically — OpenGL framebuffers are bottom-up
        let w = w as usize;
        let h = h as usize;
        let stride = w * 4;
        let mut flipped = vec![0u8; pixels.len()];
        for row in 0..h {
            let src = &pixels[(h - row - 1) * stride..][..stride];
            flipped[row * stride..][..stride].copy_from_slice(src);
        }

        RgbaImage::from_raw(w as u32, h as u32, flipped)
    }

    fn destroy(&self) {
        self.gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
        self.gl.delete_textures(&[self.texture_id]);
        self.gl.delete_renderbuffers(&[self.renderbuffer_id]);
        self.gl.delete_framebuffers(&[self.framebuffer_id]);
    }
}

pub struct GpuSharedRenderingContext {
    pub connection: Connection,
    pub adapter: Adapter,
    pub device: RefCell<Device>,
    pub context: RefCell<Context>,
    pub size: Cell<PhysicalSize<u32>>,
    pub gleam_gl: Rc<dyn Gl>,
    pub glow_gl: Arc<glow::Context>,
    // Option so we can take it in drop() before destroying the GL context
    framebuffer: RefCell<Option<CustomFramebuffer>>,
}

impl GpuSharedRenderingContext {
    pub fn new(size: PhysicalSize<u32>) -> Result<Self, surfman::Error> {
        let connection = Connection::new()?;
        let adapter = connection.create_adapter()?;
        let device = connection.create_device(&adapter)?;

        let flags = surfman::ContextAttributeFlags::ALPHA
            | surfman::ContextAttributeFlags::DEPTH
            | surfman::ContextAttributeFlags::STENCIL;
        let version = surfman::GLVersion { major: 3, minor: 0 };
        let context_descriptor = device.create_context_descriptor(&surfman::ContextAttributes { flags, version })?;
        let mut context = device.create_context(&context_descriptor, None)?;

        device.make_context_current(&context)?;

        // Backing surface — needed to make the GL context valid on macOS/CGL
        let surfman_size = euclid::default::Size2D::new(size.width as i32, size.height as i32);
        let surface = device.create_surface(
            &context,
            surfman::SurfaceAccess::GPUOnly,
            surfman::SurfaceType::Generic { size: surfman_size },
        )?;
        device.bind_surface_to_context(&mut context, surface).map_err(|(e, _)| e)?;

        let gleam_gl = unsafe {
            gl::GlFns::load_with(|s| device.get_proc_address(&context, s) as *const _)
        };

        let glow_gl = unsafe {
            Arc::new(glow::Context::from_loader_function(|s| {
                device.get_proc_address(&context, s) as *const _
            }))
        };

        // Create the texture-backed FBO that WebRender will render into.
        // This matches OffscreenRenderingContext::Framebuffer::new() from servo-paint-api.
        let framebuffer = CustomFramebuffer::new(gleam_gl.clone(), size);

        Ok(GpuSharedRenderingContext {
            connection,
            adapter,
            device: RefCell::new(device),
            context: RefCell::new(context),
            size: Cell::new(size),
            gleam_gl,
            glow_gl,
            framebuffer: RefCell::new(Some(framebuffer)),
        })
    }
}

impl RenderingContext for GpuSharedRenderingContext {
    fn prepare_for_rendering(&self) {
        let device = self.device.borrow();
        let context = self.context.borrow();
        let _ = device.make_context_current(&context);
        if let Some(fb) = self.framebuffer.borrow().as_ref() {
            log::debug!("[GpuCtx] prepare_for_rendering: binding FBO {}", fb.framebuffer_id);
            fb.bind();
        }
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        let device = self.device.borrow();
        let context = self.context.borrow();
        let _ = device.make_context_current(&context);
        self.framebuffer.borrow().as_ref()?.read_to_image(source_rectangle)
    }

    fn size(&self) -> PhysicalSize<u32> {
        self.size.get()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        {
            let device = self.device.borrow_mut();
            let mut context = self.context.borrow_mut();
            let _ = device.make_context_current(&context);

            if let Ok(Some(mut old_surface)) = device.unbind_surface_from_context(&mut context) {
                let _ = device.destroy_surface(&mut context, &mut old_surface);
            }

            let surfman_size = euclid::default::Size2D::new(size.width as i32, size.height as i32);
            if let Ok(new_surface) = device.create_surface(
                &context,
                surfman::SurfaceAccess::GPUOnly,
                surfman::SurfaceType::Generic { size: surfman_size },
            ) {
                let _ = device.bind_surface_to_context(&mut context, new_surface).map_err(|(e, _)| e);
            }
        } // device + context borrows released

        // Destroy old FBO (GL context still current) and create new one
        if let Some(old_fb) = self.framebuffer.borrow_mut().take() {
            old_fb.destroy();
        }
        let new_fb = CustomFramebuffer::new(self.gleam_gl.clone(), size);
        *self.framebuffer.borrow_mut() = Some(new_fb);
        self.size.set(size);
    }

    fn present(&self) {
        let device = self.device.borrow();
        let context = self.context.borrow();
        let _ = device.make_context_current(&context);
        self.gleam_gl.flush();
    }

    fn make_current(&self) -> Result<(), surfman::Error> {
        let device = self.device.borrow();
        let context = self.context.borrow();
        device.make_context_current(&context)
    }

    fn gleam_gl_api(&self) -> Rc<dyn Gl> {
        self.gleam_gl.clone()
    }

    fn glow_gl_api(&self) -> Arc<glow::Context> {
        self.glow_gl.clone()
    }

    fn connection(&self) -> Option<Connection> {
        Some(self.device.borrow().connection())
    }

    fn create_texture(
        &self,
        surface: Surface,
    ) -> Option<(SurfaceTexture, u32, euclid::default::Size2D<i32>)> {
        let device = self.device.borrow();
        let mut context = self.context.borrow_mut();
        let surface_info = device.surface_info(&surface);
        let surface_texture = device.create_surface_texture(&mut context, surface).ok()?;
        let gl_texture = device
            .surface_texture_object(&surface_texture)
            .map(|tex| tex.0.get())
            .unwrap_or(0);
        Some((surface_texture, gl_texture, surface_info.size))
    }

    fn destroy_texture(&self, surface_texture: SurfaceTexture) -> Option<Surface> {
        let device = self.device.borrow();
        let mut context = self.context.borrow_mut();
        device
            .destroy_surface_texture(&mut context, surface_texture)
            .map_err(|(e, _)| e)
            .ok()
    }
}

impl Drop for GpuSharedRenderingContext {
    fn drop(&mut self) {
        let device = self.device.borrow();
        let context = self.context.borrow();
        let _ = device.make_context_current(&context);

        // Explicitly destroy GL objects while the context is still current,
        // before we call destroy_context below.
        if let Some(fb) = self.framebuffer.borrow_mut().take() {
            fb.destroy();
        }

        drop(context);

        let mut context = self.context.borrow_mut();
        if let Ok(Some(mut surface)) = device.unbind_surface_from_context(&mut context) {
            let _ = device.destroy_surface(&mut context, &mut surface);
        }
        let _ = device.destroy_context(&mut context);
    }
}
