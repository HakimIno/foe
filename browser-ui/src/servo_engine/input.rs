// Input event translation: winit → Servo
//
// Translates winit window events (mouse, keyboard, scroll) into
// Servo's input event format for forwarding to WebView::notify_input_event()

use euclid::Point2D;
use i_slint_backend_winit::winit::event::{ElementState, MouseButton, WindowEvent};
use i_slint_backend_winit::winit::keyboard::{Key as WinitKey, KeyLocation, PhysicalKey};
use std::str::FromStr;

/// Current mouse and modifiers state tracker
pub struct InputState {
    pub cursor_x: f64,
    pub cursor_y: f64,
    /// Native window scale factor from winit.
    pub window_scale_factor: f64,
    /// Internal Servo render scale factor.
    pub render_scale_factor: f64,
    /// Active modifiers state
    pub modifiers: keyboard_types::Modifiers,
}

// Chrome height in logical pixels (TabBar 38px + Navbar 38px)
const CHROME_HEIGHT_LOGICAL: f64 = 76.0;

impl InputState {
    pub fn new() -> Self {
        InputState {
            cursor_x: 0.0,
            cursor_y: 0.0,
            window_scale_factor: 1.0,
            render_scale_factor: 1.0,
            modifiers: keyboard_types::Modifiers::empty(),
        }
    }

    pub fn set_scale_factors(&mut self, window_scale_factor: f64, render_scale_factor: f64) {
        self.window_scale_factor = window_scale_factor.max(0.1);
        self.render_scale_factor = render_scale_factor.max(0.1);
    }

    /// Chrome height in physical pixels — what winit cursor positions use
    fn chrome_height_physical(&self) -> f64 {
        CHROME_HEIGHT_LOGICAL * self.window_scale_factor
    }

    fn physical_to_render_scale(&self) -> f64 {
        self.render_scale_factor / self.window_scale_factor
    }

    /// Check if cursor is in the webview area (physical coordinates)
    pub fn is_in_webview_area(&self) -> bool {
        self.cursor_y >= self.chrome_height_physical()
    }

    /// Get cursor position relative to webview origin (physical pixels)
    pub fn webview_relative_position(&self) -> (f64, f64) {
        let scale = self.physical_to_render_scale();
        (
            self.cursor_x * scale,
            (self.cursor_y - self.chrome_height_physical()) * scale,
        )
    }
}

/// Translate a winit WindowEvent into a Servo InputEvent
/// Returns None if the event is not relevant (e.g., window management events)
pub fn translate_event(event: &WindowEvent, state: &mut InputState) -> Option<servo::InputEvent> {
    match event {
        // Track modifiers
        WindowEvent::ModifiersChanged(winit_mods) => {
            let mod_state = winit_mods.state();
            let mut mods = keyboard_types::Modifiers::empty();
            if mod_state.control_key() {
                mods.insert(keyboard_types::Modifiers::CONTROL);
            }
            if mod_state.shift_key() {
                mods.insert(keyboard_types::Modifiers::SHIFT);
            }
            if mod_state.alt_key() {
                mods.insert(keyboard_types::Modifiers::ALT);
            }
            if mod_state.super_key() {
                mods.insert(keyboard_types::Modifiers::META);
            }
            state.modifiers = mods;
            None
        }

        WindowEvent::CursorMoved { position, .. } => {
            state.cursor_x = position.x;
            state.cursor_y = position.y;
            if state.is_in_webview_area() {
                let (x, y) = state.webview_relative_position();
                Some(servo::InputEvent::MouseMove(servo::MouseMoveEvent::new(
                    servo::WebViewPoint::Device(Point2D::new(x as f32, y as f32)),
                )))
            } else {
                None
            }
        }

        WindowEvent::MouseInput {
            state: btn_state,
            button,
            ..
        } => {
            if !state.is_in_webview_area() {
                return None;
            }

            let foe_btn = match button {
                MouseButton::Left => servo::MouseButton::Left,
                MouseButton::Right => servo::MouseButton::Right,
                MouseButton::Middle => servo::MouseButton::Middle,
                MouseButton::Back => servo::MouseButton::Back,
                MouseButton::Forward => servo::MouseButton::Forward,
                MouseButton::Other(val) => servo::MouseButton::Other(*val),
            };

            let action = match btn_state {
                ElementState::Pressed => servo::MouseButtonAction::Down,
                ElementState::Released => servo::MouseButtonAction::Up,
            };

            let (x, y) = state.webview_relative_position();
            let point = servo::WebViewPoint::Device(Point2D::new(x as f32, y as f32));
            Some(servo::InputEvent::MouseButton(
                servo::MouseButtonEvent::new(action, foe_btn, point),
            ))
        }

        WindowEvent::MouseWheel { delta, .. } => {
            if !state.is_in_webview_area() {
                return None;
            }

            let (dx, dy) = match delta {
                i_slint_backend_winit::winit::event::MouseScrollDelta::LineDelta(x, y) => {
                    (*x as f64 * 40.0, *y as f64 * 40.0)
                }
                i_slint_backend_winit::winit::event::MouseScrollDelta::PixelDelta(pos) => {
                    let scale = state.physical_to_render_scale();
                    (pos.x * scale, pos.y * scale)
                }
            };

            let (x, y) = state.webview_relative_position();
            let point = servo::WebViewPoint::Device(Point2D::new(x as f32, y as f32));
            let wheel_delta = servo::WheelDelta {
                x: dx,
                y: dy,
                z: 0.0,
                mode: servo::WheelMode::DeltaPixel,
            };
            Some(servo::InputEvent::Wheel(servo::WheelEvent::new(
                wheel_delta,
                point,
            )))
        }

        WindowEvent::KeyboardInput {
            event: winit_key_event,
            ..
        } => {
            let key_state = match winit_key_event.state {
                ElementState::Pressed => keyboard_types::KeyState::Down,
                ElementState::Released => keyboard_types::KeyState::Up,
            };

            let key = match &winit_key_event.logical_key {
                WinitKey::Character(c) => keyboard_types::Key::Character(c.to_string()),
                WinitKey::Named(named) => {
                    let winit_str = format!("{:?}", named);
                    if let Ok(named_key) = keyboard_types::NamedKey::from_str(&winit_str) {
                        keyboard_types::Key::Named(named_key)
                    } else {
                        keyboard_types::Key::Named(keyboard_types::NamedKey::Unidentified)
                    }
                }
                _ => keyboard_types::Key::Named(keyboard_types::NamedKey::Unidentified),
            };

            let code = match winit_key_event.physical_key {
                PhysicalKey::Code(key_code) => {
                    let winit_str = format!("{:?}", key_code);
                    keyboard_types::Code::from_str(&winit_str)
                        .unwrap_or(keyboard_types::Code::Unidentified)
                }
                PhysicalKey::Unidentified(_) => keyboard_types::Code::Unidentified,
            };

            let location = match winit_key_event.location {
                KeyLocation::Standard => keyboard_types::Location::Standard,
                KeyLocation::Left => keyboard_types::Location::Left,
                KeyLocation::Right => keyboard_types::Location::Right,
                KeyLocation::Numpad => keyboard_types::Location::Numpad,
            };

            let keyboard_event = keyboard_types::KeyboardEvent {
                state: key_state,
                key,
                code,
                location,
                modifiers: state.modifiers,
                repeat: winit_key_event.repeat,
                is_composing: false,
            };

            Some(servo::InputEvent::Keyboard(servo::KeyboardEvent {
                event: keyboard_event,
            }))
        }

        _ => None,
    }
}
