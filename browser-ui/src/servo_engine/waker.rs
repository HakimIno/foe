// EventLoopWaker implementation for bridging Servo → Slint event loop
//
// Servo runs internal threads (script, layout, networking) that need to
// wake up the main UI event loop when they have work ready. This waker
// uses `slint::invoke_from_event_loop()` to safely trigger repaints
// from any Servo thread.

use servo::EventLoopWaker;

/// Waker that bridges Servo's background threads to Slint's event loop
#[derive(Clone)]
pub struct SlintWaker;

impl SlintWaker {
    pub fn new() -> Self {
        SlintWaker
    }
}

impl EventLoopWaker for SlintWaker {
    fn wake(&self) {
        // Servo calls this from background threads when it has work ready.
        // We use slint::invoke_from_event_loop to safely schedule work on
        // the main thread.
        let _ = slint::invoke_from_event_loop(|| {
            // This triggers Slint to process pending events
            log::trace!("[SlintWaker] Servo wake-up triggered");
        });
    }

    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
}

// EventLoopWaker requires Send + Sync
unsafe impl Send for SlintWaker {}
unsafe impl Sync for SlintWaker {}
