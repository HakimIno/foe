use dpi::PhysicalSize;
use gleam::gl::{self, Gl};
use image::RgbaImage;
use servo::{DeviceIntRect, RenderingContext};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use surfman::{Adapter, Connection, Context, Device, Surface, SurfaceTexture};

#[cfg(target_os = "macos")]
pub mod macos_iosurface {
    use std::ffi::c_void;
    use std::ptr;

    // CoreFoundation types
    pub type CFTypeRef = *const c_void;
    pub type CFStringRef = *const c_void;
    pub type CFNumberRef = *const c_void;
    pub type CFDictionaryRef = *const c_void;
    pub type IOSurfaceRef = *const c_void;

    // CFNumber types
    pub const K_CF_NUMBER_S_INT32_TYPE: i32 = 3;

    // CFString encoding
    pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFStringCreateWithCString(
            allocator: CFTypeRef,
            c_str: *const std::ffi::c_char,
            encoding: u32,
        ) -> CFStringRef;
        pub fn CFNumberCreate(
            allocator: CFTypeRef,
            the_type: i32,
            value_ptr: *const c_void,
        ) -> CFNumberRef;
        pub fn CFDictionaryCreate(
            allocator: CFTypeRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            num_values: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CFDictionaryRef;
        pub fn CFRelease(obj: CFTypeRef);
    }

    #[link(name = "IOSurface", kind = "framework")]
    extern "C" {
        pub fn IOSurfaceCreate(properties: CFDictionaryRef) -> IOSurfaceRef;
    }

    // CGL TexImageIOSurface2D
    #[link(name = "OpenGL", kind = "framework")]
    extern "C" {
        pub fn CGLTexImageIOSurface2D(
            ctx: cgl::CGLContextObj,
            target: u32,
            internal_format: u32,
            width: i32,
            height: i32,
            format: u32,
            gl_type: u32,
            ioSurface: IOSurfaceRef,
            plane: u32,
        ) -> cgl::CGLError;
    }

    pub unsafe fn create_iosurface(width: u32, height: u32) -> Option<IOSurfaceRef> {
        let key_width = CFStringCreateWithCString(
            ptr::null(),
            b"IOSurfaceWidth\0".as_ptr() as *const _,
            K_CF_STRING_ENCODING_UTF8,
        );
        let key_height = CFStringCreateWithCString(
            ptr::null(),
            b"IOSurfaceHeight\0".as_ptr() as *const _,
            K_CF_STRING_ENCODING_UTF8,
        );
        let key_bytes_per_elem = CFStringCreateWithCString(
            ptr::null(),
            b"IOSurfaceBytesPerElement\0".as_ptr() as *const _,
            K_CF_STRING_ENCODING_UTF8,
        );
        let key_pixel_format = CFStringCreateWithCString(
            ptr::null(),
            b"IOSurfacePixelFormat\0".as_ptr() as *const _,
            K_CF_STRING_ENCODING_UTF8,
        );

        let w_val = width as i32;
        let val_width = CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_S_INT32_TYPE,
            &w_val as *const _ as *const _,
        );

        let h_val = height as i32;
        let val_height = CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_S_INT32_TYPE,
            &h_val as *const _ as *const _,
        );

        let bpe_val = 4i32;
        let val_bytes_per_elem = CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_S_INT32_TYPE,
            &bpe_val as *const _ as *const _,
        );

        let fmt_val = 1111970369i32; // 'BGRA' FourCC
        let val_pixel_format = CFNumberCreate(
            ptr::null(),
            K_CF_NUMBER_S_INT32_TYPE,
            &fmt_val as *const _ as *const _,
        );

        let keys = [key_width, key_height, key_bytes_per_elem, key_pixel_format];
        let values = [val_width, val_height, val_bytes_per_elem, val_pixel_format];

        let dict = CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            keys.len() as isize,
            ptr::null(),
            ptr::null(),
        );

        if dict.is_null() {
            log::error!("[IOSurface] Failed to create CFDictionary properties");
            for &k in &keys {
                if !k.is_null() {
                    CFRelease(k);
                }
            }
            for &v in &values {
                if !v.is_null() {
                    CFRelease(v);
                }
            }
            return None;
        }

        let iosurface = IOSurfaceCreate(dict);

        // Clean up CF objects
        CFRelease(dict);
        for &k in &keys {
            CFRelease(k);
        }
        for &v in &values {
            CFRelease(v);
        }

        if iosurface.is_null() {
            log::error!("[IOSurface] IOSurfaceCreate returned null");
            None
        } else {
            log::debug!("[IOSurface] Created IOSurface {}x{}", width, height);
            Some(iosurface)
        }
    }
}

/// Custom texture-backed FBO that WebRender renders into.
/// Mirrors the `Framebuffer` struct in servo-paint-api's OffscreenRenderingContext.
struct CustomFramebuffer {
    gl: Rc<dyn Gl>,
    framebuffer_id: u32,
    texture_id: u32,
    renderbuffer_id: u32,
    #[cfg(target_os = "macos")]
    iosurface: macos_iosurface::IOSurfaceRef,
}

impl CustomFramebuffer {
    fn new(gl: Rc<dyn Gl>, size: PhysicalSize<u32>) -> Self {
        let fbo = gl.gen_framebuffers(1)[0];
        gl.bind_framebuffer(gl::FRAMEBUFFER, fbo);

        let tex = gl.gen_textures(1)[0];

        #[cfg(target_os = "macos")]
        let target = 0x84F5; // GL_TEXTURE_RECTANGLE
        #[cfg(not(target_os = "macos"))]
        let target = gl::TEXTURE_2D;

        gl.bind_texture(target, tex);

        #[cfg(target_os = "macos")]
        let iosurface = unsafe {
            let surface = macos_iosurface::create_iosurface(size.width, size.height)
                .expect("Failed to create IOSurface");

            let ctx = cgl::CGLGetCurrentContext();
            if ctx.is_null() {
                log::error!(
                    "[GpuCtx] Current CGL context is null during CustomFramebuffer creation"
                );
            }

            // CGLTexImageIOSurface2D expects GL_TEXTURE_RECTANGLE, RGBA internal format, BGRA format, and UNSIGNED_INT_8_8_8_8_REV type.
            let err = macos_iosurface::CGLTexImageIOSurface2D(
                ctx,
                0x84F5, // GL_TEXTURE_RECTANGLE
                gl::RGBA as u32,
                size.width as i32,
                size.height as i32,
                gl::BGRA,
                0x8367, // GL_UNSIGNED_INT_8_8_8_8_REV
                surface,
                0,
            );

            if err != 0 {
                log::error!("[GpuCtx] CGLTexImageIOSurface2D failed with error: {}", err);
            }
            surface
        };

        #[cfg(not(target_os = "macos"))]
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

        gl.tex_parameter_i(target, gl::TEXTURE_MAG_FILTER, gl::NEAREST as gl::GLint);
        gl.tex_parameter_i(target, gl::TEXTURE_MIN_FILTER, gl::NEAREST as gl::GLint);
        gl.framebuffer_texture_2d(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, target, tex, 0);
        gl.bind_texture(target, 0);

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
            log::debug!(
                "[GpuCtx] Created FBO {} ({}x{})",
                fbo,
                size.width,
                size.height
            );
        }

        Self {
            gl,
            framebuffer_id: fbo,
            texture_id: tex,
            renderbuffer_id: rbo,
            #[cfg(target_os = "macos")]
            iosurface,
        }
    }

    fn bind(&self) {
        self.gl
            .bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id);
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        self.gl
            .bind_framebuffer(gl::FRAMEBUFFER, self.framebuffer_id);
        self.gl.bind_vertex_array(0);
        self.gl.finish();

        let x = source_rectangle.min.x;
        let y = source_rectangle.min.y;
        let w = source_rectangle.width();
        let h = source_rectangle.height();

        let pixels = self.gl.read_pixels(x, y, w, h, gl::RGBA, gl::UNSIGNED_BYTE);

        let gl_err = self.gl.get_error();
        if gl_err != gl::NO_ERROR {
            log::warn!(
                "[GpuCtx] GL error 0x{:x} after read_pixels (fbo={})",
                gl_err,
                self.framebuffer_id
            );
        }

        if log::log_enabled!(log::Level::Debug) && pixels.len() >= 16 {
            let cx = w as usize / 2;
            let cy = h as usize / 2;
            let ci = (cy * w as usize + cx) * 4;
            let cp = if ci + 3 < pixels.len() {
                [pixels[ci], pixels[ci + 1], pixels[ci + 2], pixels[ci + 3]]
            } else {
                [0, 0, 0, 0]
            };
            log::debug!(
                "[GpuCtx] {}x{} corner=[{},{},{},{}] center=[{},{},{},{}]",
                w,
                h,
                pixels[0],
                pixels[1],
                pixels[2],
                pixels[3],
                cp[0],
                cp[1],
                cp[2],
                cp[3]
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
        #[cfg(target_os = "macos")]
        unsafe {
            macos_iosurface::CFRelease(self.iosurface);
        }
    }
}

/// Number of IOSurface-backed render targets we rotate through. Two is
/// enough for producer/consumer double-buffering: Servo renders into one
/// surface while Slint samples the other, so the two contexts never touch
/// the same surface at the same time.
const BUFFER_COUNT: usize = 2;

pub struct GpuSharedRenderingContext {
    pub connection: Connection,
    pub adapter: Adapter,
    pub device: RefCell<Device>,
    pub context: RefCell<Context>,
    pub size: Cell<PhysicalSize<u32>>,
    pub gleam_gl: Rc<dyn Gl>,
    pub glow_gl: Arc<glow::Context>,
    // Double-buffered render targets. Empty only after teardown (Drop takes
    // them while the GL context is still current). Servo renders into
    // `render_index`; the consumer reads the most recently presented
    // surface via `front_index`.
    framebuffers: RefCell<Vec<CustomFramebuffer>>,
    // Surface Servo renders the *next* frame into.
    render_index: Cell<usize>,
    // Most recently fully-presented surface — the one the consumer samples.
    front_index: Cell<usize>,
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
        let context_descriptor =
            device.create_context_descriptor(&surfman::ContextAttributes { flags, version })?;
        let mut context = device.create_context(&context_descriptor, None)?;

        device.make_context_current(&context)?;

        // Backing surface — needed to make the GL context valid on macOS/CGL
        let surfman_size = euclid::default::Size2D::new(size.width as i32, size.height as i32);
        let surface = device.create_surface(
            &context,
            surfman::SurfaceAccess::GPUOnly,
            surfman::SurfaceType::Generic { size: surfman_size },
        )?;
        device
            .bind_surface_to_context(&mut context, surface)
            .map_err(|(e, _)| e)?;

        let gleam_gl =
            unsafe { gl::GlFns::load_with(|s| device.get_proc_address(&context, s) as *const _) };

        let glow_gl = unsafe {
            Arc::new(glow::Context::from_loader_function(|s| {
                device.get_proc_address(&context, s) as *const _
            }))
        };

        // Create the texture-backed FBOs that WebRender renders into — one
        // per buffer. Each mirrors OffscreenRenderingContext::Framebuffer
        // from servo-paint-api, just rotated for double-buffering.
        let mut framebuffers = Vec::with_capacity(BUFFER_COUNT);
        for _ in 0..BUFFER_COUNT {
            framebuffers.push(CustomFramebuffer::new(gleam_gl.clone(), size));
        }

        Ok(GpuSharedRenderingContext {
            connection,
            adapter,
            device: RefCell::new(device),
            context: RefCell::new(context),
            size: Cell::new(size),
            gleam_gl,
            glow_gl,
            framebuffers: RefCell::new(framebuffers),
            render_index: Cell::new(0),
            front_index: Cell::new(0),
        })
    }

    pub fn get_iosurface(&self) -> Option<*const std::ffi::c_void> {
        #[cfg(target_os = "macos")]
        {
            // The consumer reads the most recently presented surface.
            self.framebuffers
                .borrow()
                .get(self.front_index.get())
                .map(|fb| fb.iosurface)
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }
}

impl RenderingContext for GpuSharedRenderingContext {
    fn prepare_for_rendering(&self) {
        let device = self.device.borrow();
        let context = self.context.borrow();
        let _ = device.make_context_current(&context);
        // Render into the back buffer — never the surface the consumer is
        // currently sampling.
        if let Some(fb) = self.framebuffers.borrow().get(self.render_index.get()) {
            log::debug!(
                "[GpuCtx] prepare_for_rendering: binding FBO {} (buffer {})",
                fb.framebuffer_id,
                self.render_index.get()
            );
            fb.bind();
        }
    }

    fn read_to_image(&self, source_rectangle: DeviceIntRect) -> Option<RgbaImage> {
        let device = self.device.borrow();
        let context = self.context.borrow();
        let _ = device.make_context_current(&context);
        // CPU read-back samples the presented (front) surface.
        self.framebuffers
            .borrow()
            .get(self.front_index.get())?
            .read_to_image(source_rectangle)
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
                let _ = device
                    .bind_surface_to_context(&mut context, new_surface)
                    .map_err(|(e, _)| e);
            }
        } // device + context borrows released

        // Destroy old FBOs (GL context still current) and recreate the full
        // buffer set at the new size.
        {
            let mut fbs = self.framebuffers.borrow_mut();
            for fb in fbs.drain(..) {
                fb.destroy();
            }
            for _ in 0..BUFFER_COUNT {
                fbs.push(CustomFramebuffer::new(self.gleam_gl.clone(), size));
            }
        }
        self.render_index.set(0);
        self.front_index.set(0);
        self.size.set(size);
    }

    fn present(&self) {
        let device = self.device.borrow();
        let context = self.context.borrow();
        let _ = device.make_context_current(&context);

        // Block until Servo's render commands for this frame have fully
        // completed on the GPU before the frame counts as presented.
        //
        // The consumer (Slint) samples our shared surface from a *separate*
        // GL context. A bare flush() only submits commands without waiting,
        // so Slint's blit could run mid-render and sample a partial or
        // freshly-cleared surface — the visible flicker. finish() drains the
        // pipeline so the surface is stable by the time the frame-ready
        // signal reaches the compositor. This mirrors the finish() the CPU
        // read-back path already does before glReadPixels (see read_to_image).
        //
        // NOTE: a glFenceSync + glClientWaitSync pair would be lighter than a
        // full finish(), but sync objects are GL 3.2 / ARB_sync while this
        // context is created at GL 3.0 — calling an unloaded entry point would
        // panic. Revisit once the context is bumped to >= 3.2 to drop this
        // CPU stall entirely.
        self.gleam_gl.finish();

        // Promote the surface we just rendered (and finished) to "front" so
        // the consumer reads a complete frame, and route the next frame to
        // the other buffer. Combined with double-buffering this removes the
        // read-during-write race that a single shared surface can't avoid
        // even with finish(): the consumer's `front` is never the surface
        // Servo writes next.
        let rendered = self.render_index.get();
        self.front_index.set(rendered);
        self.render_index.set((rendered + 1) % BUFFER_COUNT);
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
        for fb in self.framebuffers.borrow_mut().drain(..) {
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
