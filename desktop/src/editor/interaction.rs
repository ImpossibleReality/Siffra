use crate::ui::easing::cubic_bezier;
use crate::ui::theme;
use gpui::Pixels;
use std::ops::Range;
use std::time::Instant;

pub(super) struct Fade {
    pub(super) from: f32,
    pub(super) to: f32,
    pub(super) started: Instant,
}

impl Fade {
    pub(super) fn new(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            started: Instant::now(),
        }
    }

    pub(super) fn set(&mut self, to: f32) {
        if self.to == to {
            return;
        }
        self.from = self.value();
        self.to = to;
        self.started = Instant::now();
    }

    pub(super) fn value(&self) -> f32 {
        let progress =
            (self.started.elapsed().as_secs_f32() / theme::SEPARATOR_FADE).clamp(0.0, 1.0);
        let eased = progress * progress * (3.0 - 2.0 * progress);
        self.from + (self.to - self.from) * eased
    }

    pub(super) fn settled(&self) -> bool {
        self.started.elapsed().as_secs_f32() >= theme::SEPARATOR_FADE
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Granularity {
    Character,
    Word,
    Line,
}

pub(super) struct Drag {
    pub(super) granularity: Granularity,
    pub(super) anchor: Range<usize>,
}

#[derive(Clone, Copy)]
pub(super) struct Scrollbar {
    pub(super) track_top: Pixels,
    pub(super) track_height: Pixels,
    pub(super) thumb_top: Pixels,
    pub(super) thumb_height: Pixels,
    pub(super) scroll_max: Pixels,
}

#[derive(Clone, Copy)]
pub(super) struct CopyFlash {
    pub(super) result: usize,
    pub(super) at: Instant,
}

impl CopyFlash {
    pub(super) fn lit(&self) -> bool {
        self.at.elapsed().as_secs_f32() < theme::COPY_FLASH
    }

    pub(super) fn scale(&self) -> f32 {
        let elapsed = self.at.elapsed().as_secs_f32();
        let leg = theme::COPY_SQUISH;
        let (from, to, progress) = if elapsed < leg {
            (1.0, theme::COPY_SCALE, elapsed / leg)
        } else if elapsed < leg * 2.0 {
            (theme::COPY_SCALE, 1.0, (elapsed - leg) / leg)
        } else {
            return 1.0;
        };

        let (x1, y1, x2, y2) = theme::COPY_EASE;
        from + (to - from) * cubic_bezier(x1, y1, x2, y2, progress)
    }
}
