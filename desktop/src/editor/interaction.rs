//! Pointer, selection, and transient visual interaction state.

use crate::ui::easing::cubic_bezier;
use crate::ui::theme;
use gpui::Pixels;
use std::ops::Range;
use std::time::Instant;

/// How the separator's fade stands between hidden and shown.
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
        // Smoothstep gives this short fade a gentle start and end.
        let eased = progress * progress * (3.0 - 2.0 * progress);
        self.from + (self.to - self.from) * eased
    }

    pub(super) fn settled(&self) -> bool {
        self.started.elapsed().as_secs_f32() >= theme::SEPARATOR_FADE
    }
}

/// How much of the text a drag takes at a time, set by the click that started
/// it: a plain click selects to the character, a double click keeps whole words
/// in the selection, and a triple click keeps whole lines.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Granularity {
    Character,
    Word,
    Line,
}

/// A selection being dragged out with the button held down.
pub(super) struct Drag {
    pub(super) granularity: Granularity,
    /// What the click that began the drag selected — a caret, a word or a line.
    /// The selection always covers this, however far back over it the pointer
    /// is dragged, which is what stops a double-click drag from losing the word
    /// it started on.
    pub(super) anchor: Range<usize>,
}

/// Where the scrollbar was last drawn, so a drag on its thumb can be turned
/// back into a scroll offset. All in window coordinates.
#[derive(Clone, Copy)]
pub(super) struct Scrollbar {
    pub(super) track_top: Pixels,
    pub(super) track_height: Pixels,
    pub(super) thumb_top: Pixels,
    pub(super) thumb_height: Pixels,
    /// How far the content can scroll, which is the travel the thumb's own
    /// range of movement maps onto.
    pub(super) scroll_max: Pixels,
}

/// A result's copy animation and the moment it started.
///
/// Both effects are read from the clock, so a new copy only needs to replace
/// the start time.
#[derive(Clone, Copy)]
pub(super) struct CopyFlash {
    pub(super) result: usize,
    pub(super) at: Instant,
}

impl CopyFlash {
    /// Whether the result is still holding the colour a copy leaves behind.
    pub(super) fn lit(&self) -> bool {
        self.at.elapsed().as_secs_f32() < theme::COPY_FLASH
    }

    /// How far into the squish the result is, down to `scale(0.94)` and back.
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
