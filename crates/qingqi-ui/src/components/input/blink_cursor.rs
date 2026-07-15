//! Blinking cursor state management.

use std::time::Duration;

use gpui::{Context, Task, Timer};

static INTERVAL: Duration = Duration::from_millis(500);
static PAUSE_DELAY: Duration = Duration::from_millis(300);

/// Manages the Input cursor blinking.
///
/// It will start blinking with a interval of 500ms.
/// Every loop will notify the view to update the `visible`, and Input will observe this update to trigger repaint.
pub(crate) struct BlinkCursor {
    visible: bool,
    paused: bool,
    epoch: usize,
    _task: Task<()>,
}

impl BlinkCursor {
    pub fn new() -> Self {
        Self {
            visible: false,
            paused: false,
            epoch: 0,
            _task: Task::ready(()),
        }
    }

    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.visible = true;
        cx.notify();

        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            Timer::after(PAUSE_DELAY).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| this.blink(epoch, cx)).ok();
            }
        });
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.epoch = 0;
        cx.notify();
    }

    fn next_epoch(&mut self) -> usize {
        self.epoch += 1;
        self.epoch
    }

    fn blink(&mut self, epoch: usize, cx: &mut Context<Self>) {
        if self.paused || epoch != self.epoch {
            self.visible = true;
            return;
        }

        self.visible = !self.visible;
        cx.notify();

        // Schedule the next blink
        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            Timer::after(INTERVAL).await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| this.blink(epoch, cx)).ok();
            }
        });
    }

    pub fn visible(&self) -> bool {
        // Keep showing the cursor if paused
        self.paused || self.visible
    }

    pub fn is_running(&self) -> bool {
        self.epoch > 0
    }

    /// Pause the blinking, and delay 300ms to resume the blinking.
    pub fn pause(&mut self, cx: &mut Context<Self>) {
        self.paused = true;
        self.visible = true;
        cx.notify();

        // delay 300ms to start the blinking
        let epoch = self.next_epoch();
        self._task = cx.spawn(async move |this, cx| {
            Timer::after(PAUSE_DELAY).await;

            if let Some(this) = this.upgrade() {
                this.update(cx, |this, cx| {
                    this.paused = false;
                    this.blink(epoch, cx);
                })
                .ok();
            }
        });
    }
}
