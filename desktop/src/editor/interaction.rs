use super::Editor;
use super::layout::Layout;
use crate::ui::easing::cubic_bezier;
use crate::ui::theme;
use gpui::{Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Window, px};
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

impl Editor {
    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.preferred_x = None;
        self.text.close_history_group();
        let offset = self.offset_for_position(event.position);

        let (granularity, anchor) = match event.click_count {
            0 | 1 if event.modifiers.shift => {
                let fixed = if self.text.selection_reversed() {
                    self.text.selection().end
                } else {
                    self.text.selection().start
                };
                (Granularity::Character, fixed..fixed)
            }
            0 | 1 => (Granularity::Character, offset..offset),
            2 => (Granularity::Word, self.word_range_at(offset)),
            _ => (Granularity::Line, self.line_range_at(offset)),
        };

        self.drag = Some(Drag {
            granularity,
            anchor,
        });
        self.drag_to(offset, cx);
    }

    fn drag_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };

        let reach = match drag.granularity {
            Granularity::Character => offset..offset,
            Granularity::Word => self.word_range_at(offset),
            Granularity::Line => self.line_range_at(offset),
        };
        let anchor = drag.anchor.clone();

        let selection = anchor.start.min(reach.start)..anchor.end.max(reach.end);
        self.text
            .set_selection(selection, reach.start < anchor.start);
        self.caret_moved(cx);
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pointer_leads = true;

        if self.drag.is_some() {
            let offset = self.offset_for_position(event.position);
            self.drag_to(offset, cx);
        }

        if self.dragging_separator {
            let width = f32::from(
                self.layout
                    .as_ref()
                    .map_or(px(1.0), |layout| layout.metrics.window_width),
            );
            self.output_fraction = (1.0 - f32::from(event.position.x) / width).clamp(0.0, 1.0);
            cx.notify();
        }

        if let (Some(grab), Some(bar)) = (self.scrollbar_grab, self.scrollbar) {
            self.scroll_to_thumb_top(event.position.y - grab, bar);
            cx.notify();
        }
    }

    pub(super) fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some()
            || self.dragging_separator
            || self.scrollbar_grab.is_some()
            || self.pressed_result.is_some()
        {
            self.drag = None;
            self.dragging_separator = false;
            self.scrollbar_grab = None;
            self.pressed_result = None;
            cx.notify();
        }
    }

    pub(super) fn scroll_caret_into_view(
        &self,
        layout: &Layout,
        view_height: Pixels,
        content_height: Pixels,
    ) {
        let Some(position) = layout.position_for_offset(self.caret_offset()) else {
            return;
        };
        if view_height <= px(0.0) {
            return;
        }

        let margin = px(theme::GUTTER);
        let top = px(theme::GUTTER) + position.y;
        let bottom = top + layout.metrics.line_height;

        let mut offset = self.scroll.offset();
        let view_top = -offset.y;
        if top - margin < view_top {
            offset.y = -(top - margin);
        } else if bottom + margin > view_top + view_height {
            offset.y = view_height - (bottom + margin);
        } else {
            return;
        }

        let scroll_max = (content_height - view_height).max(px(0.0));
        offset.y = offset.y.clamp(-scroll_max, px(0.0));
        self.scroll.set_offset(offset);
    }

    fn scroll_to_thumb_top(&self, top: Pixels, bar: Scrollbar) {
        let travel = bar.track_height - bar.thumb_height;
        let progress = if travel > px(0.0) {
            (f32::from(top - bar.track_top) / f32::from(travel)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut offset = self.scroll.offset();
        offset.y = -bar.scroll_max * progress;
        self.scroll.set_offset(offset);
    }

    pub(super) fn preserve_scroll_position_for_font_resize(
        &self,
        content_height: Pixels,
        view_height: Pixels,
    ) {
        let Some(previous) = self.scrollbar else {
            return;
        };

        let progress =
            (f32::from(-self.scroll.offset().y) / f32::from(previous.scroll_max)).clamp(0.0, 1.0);
        let scroll_max = (content_height - view_height).max(px(0.0));
        let mut offset = self.scroll.offset();
        offset.y = -scroll_max * progress;
        self.scroll.set_offset(offset);
    }

    pub(super) fn measure_scrollbar(&mut self, view_height: Pixels, content_height: Pixels) {
        let scroll_max = content_height - view_height;
        if scroll_max <= px(0.5) || view_height <= px(0.0) {
            self.scrollbar = None;
            return;
        }

        let inset = px(theme::SCROLLBAR_INSET);
        let foot = inset + px(theme::corner_clearance(theme::SCROLLBAR_INSET));
        let track_height = (view_height - inset - foot).max(px(0.0));
        let thumb_height = (track_height * (f32::from(view_height) / f32::from(content_height)))
            .max(px(theme::SCROLLBAR_MIN_THUMB))
            .min(track_height);

        let progress = (f32::from(-self.scroll.offset().y) / f32::from(scroll_max)).clamp(0.0, 1.0);
        let track_top = px(theme::NAV_HEIGHT) + inset;

        self.scrollbar = Some(Scrollbar {
            track_top,
            track_height,
            thumb_top: track_top + (track_height - thumb_height) * progress,
            thumb_height,
            scroll_max,
        });
    }
}
