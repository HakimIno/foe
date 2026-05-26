// Rendering utilities for Servo → Slint display bridge

use slint::Image;

/// Create a placeholder frame (dark background matching the UI)
/// Used before Servo finishes rendering the first frame
pub fn create_placeholder_frame(width: u32, height: u32) -> Image {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = 0x1E; // R
        chunk[1] = 0x1F; // G
        chunk[2] = 0x22; // B
        chunk[3] = 0xFF; // A
    }
    let pixel_buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        &pixels,
        width,
        height,
    );
    Image::from_rgba8(pixel_buffer)
}
