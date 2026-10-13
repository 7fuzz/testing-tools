use std::sync::mpsc;
use eframe::egui;

/// Work on a background thread; the UI polls for the result each frame.
pub struct Job<T> {
    rx: mpsc::Receiver<T>,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(ctx: &egui::Context, work: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
            ctx.request_repaint();
        });
        Self { rx }
    }

    pub fn poll(&self) -> Option<T> {
        self.rx.try_recv().ok()
    }
}

/// Polls `slot`'s job, clearing the slot once its result arrives.
pub fn take_finished<T: Send + 'static>(slot: &mut Option<Job<T>>) -> Option<T> {
    let result = slot.as_ref()?.poll()?;
    *slot = None;
    Some(result)
}
