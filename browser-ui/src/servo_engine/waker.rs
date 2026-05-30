// EventLoopWaker implementation for bridging Servo → Slint event loop
//
// Servo runs internal threads (script, layout, networking) that need to
// wake up the main UI event loop when they have work ready. This waker
// uses `slint::invoke_from_event_loop()` to safely trigger repaints
// from any Servo thread.

use servo::EventLoopWaker;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Waker that bridges Servo's background threads to Slint's event loop.
/// Uses atomic coalescing to prevent event loop saturation.
#[derive(Clone)]
pub struct SlintWaker {
    is_woken: Arc<AtomicBool>,
}

impl SlintWaker {
    pub fn new() -> Self {
        SlintWaker {
            is_woken: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl EventLoopWaker for SlintWaker {
    fn wake(&self) {
        // If a wake callback is already pending on the Slint event loop,
        // we skip queuing a new one to avoid flooding the main thread.
        if self.is_woken.swap(true, Ordering::SeqCst) {
            log::trace!("[SlintWaker] Wake request coalesced (already pending)");
            return;
        }

        let is_woken_clone = self.is_woken.clone();
        let _ = slint::invoke_from_event_loop(move || {
            // Reset the flag *before* we pump the event loop so that
            // subsequent wakeups during or after the pump can be scheduled.
            is_woken_clone.store(false, Ordering::SeqCst);

            log::trace!("[SlintWaker] Servo wake-up triggered");
            crate::rendering_setup::trigger_pump();
        });
    }

    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
}

// EventLoopWaker requires Send + Sync
unsafe impl Send for SlintWaker {}
unsafe impl Sync for SlintWaker {}
