//! The editor: a column of input, a column of results, and the rulers that tie
//! one to the other.
//!
//! The text is a plain `String` addressed by plain byte offsets, so there is no
//! document model to keep in step with what is on screen. Everything else —
//! where a line wrapped, how tall it stands, how much of a result fits beside
//! it — is derived from that string once per frame, which is what keeps a
//! result sitting against the line it belongs to.

mod element;
pub mod layout;
mod result;

use crate::diagnostics::{self, should_hide_diagnostic};
use crate::engine::{Calculation, LineOutput};
use crate::text_input::{
    Backspace, Copy, Cut, Delete, DeleteToLineStart, DeleteWordLeft, DeleteWordRight, EditKind,
    End, Home, Left, Paste, Redo, Right, SelectAll, SelectEnd, SelectHome, SelectLeft, SelectRight,
    SelectWordLeft, SelectWordRight, TextBuffer, Undo, WordLeft, WordRight,
};
use crate::theme;
use crate::toolbar::Toolbar;
use crate::ui::easing::cubic_bezier;
use crate::ui::popover::{below_right, popover_button};
use crate::ui::toast::Toast;
use element::InputElement;
use layout::{Fonts, Layout, Metrics};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ClickEvent, ClipboardItem, Context, CursorStyle, Div, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Task, UTF16Selection, Window,
    actions, div, point, prelude::FluentBuilder, px,
};

/// Carries each line's last-edit time across an edit.
///
/// The lines the edit did not touch are found by matching the old text against
/// the new from both ends, and keep the time they had; everything between the
/// two runs counts as just edited. Matching from both ends is what makes
/// inserting a line near the top cost only that line: were the times simply
/// indexed, every line below the insertion would shift by one and read as
/// freshly typed, and the whole calculation would fall silent at once.
fn carry_edit_times<T: std::marker::Copy>(
    previous: &[String],
    times: &[Option<T>],
    next: &[String],
    now: T,
) -> Vec<Option<T>> {
    let mut prefix = 0;
    while prefix < previous.len() && prefix < next.len() && previous[prefix] == next[prefix] {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < previous.len() - prefix
        && suffix < next.len() - prefix
        && previous[previous.len() - 1 - suffix] == next[next.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let mut edited = vec![Some(now); next.len()];
    for index in 0..prefix {
        edited[index] = times.get(index).copied().flatten();
    }
    for offset in 0..suffix {
        edited[next.len() - 1 - offset] = times.get(previous.len() - 1 - offset).copied().flatten();
    }
    edited
}

// What only a column of text can do. The rest of the editor's commands are the
// ones every field shares, and live in `text_input`.
actions!(
    siffra,
    [
        Up,
        Down,
        SelectUp,
        SelectDown,
        DocumentStart,
        DocumentEnd,
        Newline,
        ShowCharacterPalette,
    ]
);

/// How the separator's fade stands between hidden and shown.
struct Fade {
    from: f32,
    to: f32,
    started: Instant,
}

impl Fade {
    fn new(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            started: Instant::now(),
        }
    }

    fn set(&mut self, to: f32) {
        if self.to == to {
            return;
        }
        self.from = self.value();
        self.to = to;
        self.started = Instant::now();
    }

    fn value(&self) -> f32 {
        let progress =
            (self.started.elapsed().as_secs_f32() / theme::SEPARATOR_FADE).clamp(0.0, 1.0);
        // Smoothstep gives this short fade a gentle start and end.
        let eased = progress * progress * (3.0 - 2.0 * progress);
        self.from + (self.to - self.from) * eased
    }

    fn settled(&self) -> bool {
        self.started.elapsed().as_secs_f32() >= theme::SEPARATOR_FADE
    }
}

/// How much of the text a drag takes at a time, set by the click that started
/// it: a plain click selects to the character, a double click keeps whole words
/// in the selection, and a triple click keeps whole lines.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Granularity {
    Character,
    Word,
    Line,
}

/// A selection being dragged out with the button held down.
struct Drag {
    granularity: Granularity,
    /// What the click that began the drag selected — a caret, a word or a line.
    /// The selection always covers this, however far back over it the pointer
    /// is dragged, which is what stops a double-click drag from losing the word
    /// it started on.
    anchor: Range<usize>,
}

/// Where the scrollbar was last drawn, so a drag on its thumb can be turned
/// back into a scroll offset. All in window coordinates.
#[derive(Clone, Copy)]
struct Scrollbar {
    track_top: Pixels,
    track_height: Pixels,
    thumb_top: Pixels,
    thumb_height: Pixels,
    /// How far the content can scroll, which is the travel the thumb's own
    /// range of movement maps onto.
    scroll_max: Pixels,
}

/// A result's copy animation and the moment it started.
///
/// Both effects are read from the clock, so a new copy only needs to replace
/// the start time.
#[derive(Clone, Copy)]
struct CopyFlash {
    result: usize,
    at: Instant,
}

impl CopyFlash {
    /// Whether the result is still holding the colour a copy leaves behind.
    fn lit(&self) -> bool {
        self.at.elapsed().as_secs_f32() < theme::COPY_FLASH
    }

    /// How far into the squish the result is, down to `scale(0.94)` and back.
    fn scale(&self) -> f32 {
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

/// Emitted whenever the text changes, which is the workspace's cue that there is
/// something new to save.
pub struct ContentChanged;

pub struct Editor {
    focus_handle: FocusHandle,
    /// The top bar, positioned from the same window corner as the editor's
    /// other absolute elements.
    toolbar: Entity<Toolbar>,
    /// The only copy of the text, its selection, composition, and undo stack.
    pub(crate) text: TextBuffer,
    /// The column an up or down keypress aims for, so a run of them tracks
    /// straight down past short lines instead of drifting in from their ends.
    preferred_x: Option<Pixels>,
    drag: Option<Drag>,
    caret_visible: bool,
    blink: Option<Task<()>>,
    calculation: Calculation,
    /// What every line evaluates to, whether or not it is being shown yet.
    outputs: Vec<Option<LineOutput>>,
    /// What the results column is actually showing — `outputs` with diagnostics
    /// that are still settling held back.
    displayed: Vec<Option<LineOutput>>,
    /// The text as it stood at the last edit, and when each of its lines was
    /// last touched. A line is only held back from complaining about itself
    /// while it is the one being typed.
    previous_lines: Vec<String>,
    line_edited_at: Vec<Option<Instant>>,
    /// Wakes the editor when the diagnostic held back longest falls due.
    diagnostic_timer: Option<Task<()>>,
    fonts: Fonts,
    /// The share of the window the results column takes, as dragged.
    output_fraction: f32,
    dragging_separator: bool,
    separator_hovered: bool,
    separator_fade: Fade,
    /// The result's pointer state, tracked so the painted vinculum matches the
    /// color of the text under it.
    hovered_result: Option<usize>,
    pressed_result: Option<usize>,
    /// The result whose popover is showing, if any. Only ever one: opening a
    /// second closes the first, as clicking anywhere else does.
    open_result: Option<usize>,
    /// Set by a press that has already been accounted for, so the press that
    /// opens a popover — or that lands inside one — is not also taken as the
    /// press outside it that should close it.
    press_handled: bool,
    copy_flash: Option<CopyFlash>,
    toast: Toast,
    /// Where the text was last painted, for turning a click into an offset.
    text_bounds: Option<Bounds<Pixels>>,
    layout: Option<Rc<Layout>>,
    scroll: ScrollHandle,
    autoscroll: bool,
    /// `None` while the calculation fits in the window and there is nothing to
    /// scroll.
    scrollbar: Option<Scrollbar>,
    /// How far down the thumb the pointer took hold of it, so the thumb keeps
    /// its place under the pointer for the length of the drag.
    scrollbar_grab: Option<Pixels>,
}

impl EventEmitter<ContentChanged> for Editor {}

impl Editor {
    pub fn new(
        content: impl Into<String>,
        fonts: Fonts,
        toolbar: Entity<Toolbar>,
        cx: &mut Context<Self>,
    ) -> Self {
        let content = content.into();

        let mut editor = Self {
            focus_handle: cx.focus_handle(),
            toolbar,
            text: TextBuffer::new(content),
            preferred_x: None,
            drag: None,
            caret_visible: true,
            blink: None,
            calculation: Calculation::default(),
            outputs: Vec::new(),
            displayed: Vec::new(),
            previous_lines: Vec::new(),
            line_edited_at: Vec::new(),
            diagnostic_timer: None,
            fonts,
            output_fraction: theme::OUTPUT_FRACTION,
            dragging_separator: false,
            separator_hovered: false,
            separator_fade: Fade::new(0.0),
            hovered_result: None,
            pressed_result: None,
            open_result: None,
            press_handled: false,
            copy_flash: None,
            toast: Toast::new(),
            text_bounds: None,
            layout: None,
            scroll: ScrollHandle::new(),
            autoscroll: false,
            scrollbar: None,
            scrollbar_grab: None,
        };

        editor.recalculate();
        editor.refresh_diagnostics(cx);
        editor
    }

    /// Shows a short status message over the editor.
    pub fn show_toast(&mut self, message: impl Into<gpui::SharedString>, cx: &mut Context<Self>) {
        self.toast.show(message);
        cx.notify();
    }

    fn caret_offset(&self) -> usize {
        self.text.caret_offset()
    }

    pub fn content(&self) -> &str {
        self.text.content()
    }

    /// Where the caret sits, in UTF-16 code units used by platform text input.
    pub fn cursor(&self) -> usize {
        self.text.offset_to_utf16(self.caret_offset())
    }

    /// Shows another calculation. Nothing of the last one is carried over: not
    /// its undo history, not how far it was scrolled, and not which of its lines
    /// were still settling.
    pub fn set_document(&mut self, content: &str, cursor: usize, cx: &mut Context<Self>) {
        self.text.reset(content);
        let caret = self.text.offset_from_utf16(cursor);
        self.text.move_to(caret);
        self.preferred_x = None;
        self.drag = None;
        self.open_result = None;
        self.hovered_result = None;
        self.pressed_result = None;
        self.copy_flash = None;
        self.displayed = Vec::new();
        self.previous_lines = Vec::new();
        self.line_edited_at = Vec::new();
        self.diagnostic_timer = None;
        self.layout = None;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        self.recalculate();
        // Loaded diagnostics are due immediately; recalculation initializes
        // their edit times.
        self.line_edited_at = Vec::new();
        self.caret_moved(cx);
    }

    /// Hands the keyboard back to the calculation, leaving the caret where it
    /// was — which for a calculation just opened is where it was left.
    pub fn focus(&mut self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    /// Puts the caret at the end of the calculation and the keyboard with it,
    /// which is where a calculation just created or just named carries on from.
    pub fn focus_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window);
        self.move_to(self.text.content().len(), cx);
    }

    /// Re-runs the calculation. Every line is re-evaluated, but only the lines
    /// whose text or predecessors changed are actually recomputed.
    fn recalculate(&mut self) {
        let lines: Vec<String> = self
            .text
            .content()
            .split('\n')
            .map(str::to_string)
            .collect();
        self.note_edits(lines);
        self.outputs = self.calculation.evaluate(self.text.content());
    }

    fn note_edits(&mut self, next: Vec<String>) {
        self.line_edited_at = carry_edit_times(
            &self.previous_lines,
            &self.line_edited_at,
            &next,
            Instant::now(),
        );
        self.previous_lines = next;
    }

    /// How much of a line's settling time is left, or `None` once it has
    /// settled and its diagnostic may be shown.
    fn remaining_delay(&self, line: usize, now: Instant) -> Option<Duration> {
        let edited = self.line_edited_at.get(line).copied().flatten()?;
        let elapsed = now.saturating_duration_since(edited);
        (elapsed < diagnostics::ERROR_DELAY).then(|| diagnostics::ERROR_DELAY - elapsed)
    }

    /// The line the caret is on, and whether it sits at that line's end — the
    /// two things a diagnostic's visibility turns on. A selection is not a
    /// caret and suppresses nothing.
    fn caret_lines(&self) -> (Option<usize>, Option<usize>) {
        if !self.text.selection().is_empty() {
            return (None, None);
        }

        let caret = self.caret_offset();
        let mut start = 0;
        for (index, line) in self.text.content().split('\n').enumerate() {
            let end = start + line.len();
            if caret <= end {
                return (Some(index), (caret == end).then_some(index));
            }
            start = end + 1;
        }
        (None, None)
    }

    /// Decides which results the column shows.
    ///
    /// A calculation is wrong for most of the time it is being typed, and a
    /// diagnostic that appears on every keystroke is noise. An error is held
    /// back until the typing on its line has settled, and while it is held back
    /// whatever was already showing there stays put, so the column does not
    /// flicker under the caret. Values are never held back — an answer is worth
    /// having the moment there is one.
    fn refresh_diagnostics(&mut self, cx: &mut Context<Self>) {
        let lines: Vec<&str> = self.text.content().split('\n').collect();
        let (cursor_line, cursor_line_at_end) = self.caret_lines();
        let now = Instant::now();
        let previous = &self.displayed;

        let mut soonest: Option<Duration> = None;
        let mut displayed = Vec::with_capacity(self.outputs.len());

        for (index, output) in self.outputs.iter().enumerate() {
            displayed.push(match output {
                None => None,
                Some(LineOutput::Value { .. }) => output.clone(),
                Some(error @ LineOutput::Error { message, .. }) => {
                    if should_hide_diagnostic(error, index, &lines, cursor_line, cursor_line_at_end)
                    {
                        None
                    } else if message == "Incomplete expression" {
                        // The caret has already left this line — that is the
                        // only way an incomplete one gets this far — so there
                        // is nothing left to wait for.
                        Some(error.clone())
                    } else if let Some(remaining) = self.remaining_delay(index, now) {
                        soonest = Some(
                            soonest.map_or(remaining, |soonest: Duration| soonest.min(remaining)),
                        );
                        previous
                            .get(index)
                            .cloned()
                            .flatten()
                            .filter(|shown| matches!(shown, LineOutput::Error { .. }))
                    } else {
                        Some(error.clone())
                    }
                }
            });
        }

        self.displayed = displayed;
        self.diagnostic_timer = soonest.map(|delay| {
            cx.spawn(async move |editor, cx| {
                cx.background_executor().timer(delay).await;
                editor
                    .update(cx, |editor, cx| editor.refresh_diagnostics(cx))
                    .ok();
            })
        });
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.text.move_to(offset);
        self.caret_moved(cx);
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.text.select_to(offset);
        self.caret_moved(cx);
    }

    /// Restarts the blink so the caret shows solid the moment it moves, rather
    /// than possibly arriving mid-blink.
    fn caret_moved(&mut self, cx: &mut Context<Self>) {
        self.caret_visible = true;
        self.autoscroll = true;
        // Which diagnostics are worth showing turns on where the caret is, so
        // moving it can reveal one as readily as typing can.
        self.refresh_diagnostics(cx);
        self.blink = Some(cx.spawn(async move |editor, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs_f32(theme::CARET_BLINK_PERIOD / 2.0))
                    .await;
                let updated = editor.update(cx, |editor, cx| {
                    editor.caret_visible = !editor.caret_visible;
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.text.previous_boundary(offset)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.text.next_boundary(offset)
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        self.text.previous_word_boundary(offset)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        self.text.next_word_boundary(offset)
    }

    /// The word under `offset`, for a double click. A click that lands between
    /// two words takes the one before it, as a double click in a text field
    /// does.
    fn word_range_at(&self, offset: usize) -> Range<usize> {
        self.text.word_range_at(offset)
    }

    /// The whole line `offset` sits on, for a triple click.
    fn line_range_at(&self, offset: usize) -> Range<usize> {
        let Some(layout) = self.layout.as_ref() else {
            return offset..offset;
        };
        let line = &layout.lines[layout.line_index_for_offset(offset)];
        line.start..line.start + line.len
    }

    /// The offsets of the visual row `offset` sits on — the row the text
    /// wrapped onto, not the whole line, which is what Home and End address.
    fn row_bounds(&self, offset: usize) -> Option<(usize, usize)> {
        let layout = self.layout.as_ref()?;
        let position = layout.position_for_offset(offset)?;
        Some((
            layout.offset_for_position(point(px(0.0), position.y)),
            layout.offset_for_position(point(px(f32::MAX / 2.0), position.y)),
        ))
    }

    fn move_vertically(&mut self, rows: f32, extend: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.layout.clone() else {
            return;
        };
        let Some(position) = layout.position_for_offset(self.caret_offset()) else {
            return;
        };

        let x = self.preferred_x.unwrap_or(position.x);
        let line_height = layout.metrics.line_height;
        // Half a row down from the target row's top: anywhere inside the row
        // resolves to it, and the midpoint cannot be pushed into a neighbour by
        // a rounding error.
        let y = position.y + line_height * rows + line_height / 2.0;
        let offset = layout.offset_for_position(point(x, y));

        if extend {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        self.preferred_x = Some(x);
    }

    /// Replaces the selection, recording the edit under `kind` so undo can tell
    /// a run of typing from the paste that followed it.
    fn replace_selection(&mut self, text: &str, kind: EditKind, cx: &mut Context<Self>) {
        self.text.replace_selection(text, kind);
        self.text_changed(cx);
    }

    /// What every change to the text has in common: the calculation is re-run,
    /// the caret is settled, and the workspace is told there is something to
    /// save.
    fn text_changed(&mut self, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.recalculate();
        self.caret_moved(cx);
        cx.emit(ContentChanged);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.undo() {
            self.after_restore(cx);
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.redo() {
            self.after_restore(cx);
        }
    }

    fn after_restore(&mut self, cx: &mut Context<Self>) {
        self.text_changed(cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if self.text.selection().is_empty() {
            self.move_to(self.previous_boundary(self.caret_offset()), cx);
        } else {
            self.move_to(self.text.selection().start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if self.text.selection().is_empty() {
            self.move_to(self.next_boundary(self.caret_offset()), cx);
        } else {
            self.move_to(self.text.selection().end, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.select_to(self.previous_boundary(self.caret_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.select_to(self.next_boundary(self.caret_offset()), cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.move_to(self.previous_word_boundary(self.caret_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.move_to(self.next_word_boundary(self.caret_offset()), cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.select_to(self.previous_word_boundary(self.caret_offset()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.select_to(self.next_word_boundary(self.caret_offset()), cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(self.previous_word_boundary(caret)..caret);
        }
        self.replace_selection("", EditKind::Discrete, cx);
    }

    fn delete_word_right(&mut self, _: &DeleteWordRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(caret..self.next_word_boundary(caret));
        }
        self.replace_selection("", EditKind::Discrete, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1.0, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1.0, false, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1.0, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1.0, true, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((start, _)) = self.row_bounds(self.caret_offset()) {
            self.move_to(start, cx);
        }
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((_, end)) = self.row_bounds(self.caret_offset()) {
            self.move_to(end, cx);
        }
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((start, _)) = self.row_bounds(self.caret_offset()) {
            self.select_to(start, cx);
        }
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((_, end)) = self.row_bounds(self.caret_offset()) {
            self.select_to(end, cx);
        }
    }

    fn document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.move_to(0, cx);
    }

    fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.move_to(self.text.content().len(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.text.select_all();
        self.caret_moved(cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(self.previous_boundary(caret)..caret);
        }
        self.replace_selection("", EditKind::DeleteBackward, cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(caret..self.next_boundary(caret));
        }
        self.replace_selection("", EditKind::DeleteForward, cx);
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.text.selection().is_empty()
            && let Some((start, _)) = self.row_bounds(self.caret_offset())
        {
            self.text.set_selection_for_edit(start..self.caret_offset());
        }
        self.replace_selection("", EditKind::Discrete, cx);
    }

    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        self.replace_selection("\n", EditKind::Discrete, cx);
    }

    /// The value on a line, if that is what it produced. An error has nothing
    /// worth copying, and a line that produced nothing has nothing at all.
    fn value_at(&self, index: usize) -> Option<&str> {
        match self.displayed.get(index)?.as_ref()? {
            LineOutput::Value { string, .. } if !string.is_empty() => Some(string),
            _ => None,
        }
    }

    /// Takes a result, at full precision rather than as much of it as the column
    /// had room to show.
    fn copy_result(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(value) = self.value_at(index) else {
            return;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
        self.copy_flash = Some(CopyFlash {
            result: index,
            at: Instant::now(),
        });
        self.toast.show("Copied!");
        cx.notify();
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        // A result copy takes precedence while the pointer is over a result;
        // otherwise copy the selected text.
        if let Some(index) = self.hovered_result
            && self.value_at(index).is_some()
        {
            self.copy_result(index, cx);
            return;
        }

        if let Some(text) = self.text.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.text.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
            self.replace_selection("", EditKind::Discrete, cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_selection(&text, EditKind::Discrete, cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    /// A point in window coordinates as an offset into the text.
    fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.text_bounds, self.layout.as_ref()) else {
            return 0;
        };
        layout.offset_for_position(position - bounds.origin)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.preferred_x = None;
        self.text.close_history_group();
        let offset = self.offset_for_position(event.position);

        // The click that starts a drag also fixes what the drag takes at a
        // time. A second click anchors on the word under the pointer and a
        // third on the whole line, and dragging from either grows the selection
        // in those same units, the way a text field on this platform does.
        let (granularity, anchor) = match event.click_count {
            0 | 1 if event.modifiers.shift => {
                // Shift-click moves the near end of what is already selected and
                // leaves the far one where it is, so the click extends rather
                // than replaces.
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

    /// Where a drag has reached: the range the click anchored on, grown to take
    /// in whatever the pointer is over — to the character, the word or the line,
    /// depending on how the drag began.
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
        // Dragging back past where it began leaves the caret at the near end,
        // so a key pressed afterwards carries on from there.
        self.text
            .set_selection(selection, reach.start < anchor.start);
        self.caret_moved(cx);
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            let offset = self.offset_for_position(event.position);
            self.drag_to(offset, cx);
        }

        if self.dragging_separator {
            // Keep the results column's share on the right of the pointer.
            let width = f32::from(
                self.layout
                    .as_ref()
                    .map_or(px(1.0), |l| l.metrics.window_width),
            );
            self.output_fraction = (1.0 - f32::from(event.position.x) / width).clamp(0.0, 1.0);
            cx.notify();
        }

        if let (Some(grab), Some(bar)) = (self.scrollbar_grab, self.scrollbar) {
            self.scroll_to_thumb_top(event.position.y - grab, bar);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    /// Brings the caret back into view after it has moved.
    ///
    /// This runs while the frame is being laid out rather than after the text
    /// has been measured, because the scrolling container reads its offset when
    /// it prepaints — which is after this and before anything is drawn. Setting
    /// the offset any later would land on the following frame, and the frame
    /// after a keystroke is not otherwise asked for, so the caret would sit off
    /// the bottom of the window until the next blink came round to redraw it.
    ///
    /// Everything it needs is known without measuring: the viewport is the
    /// window less the title strip, and the text starts one gutter down from
    /// the top of the scrolled content.
    fn scroll_caret_into_view(&self, layout: &Layout, view_height: Pixels, content_height: Pixels) {
        let Some(position) = layout.position_for_offset(self.caret_offset()) else {
            return;
        };
        if view_height <= px(0.0) {
            return;
        }

        let margin = px(theme::GUTTER);
        // The caret's row, in the scrolled content's own coordinates.
        let top = px(theme::GUTTER) + position.y;
        let bottom = top + layout.metrics.line_height;

        let mut offset = self.scroll.offset();
        // `offset.y` is how far the content has been pushed up, and so is zero
        // or negative.
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

    /// Scrolls so the thumb's top lands at `top`, which is how a drag on the
    /// thumb is turned back into an offset.
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

    /// Works out where the scrollbar goes, and remembers it so a drag on the
    /// thumb has something to work back from. `None` while the calculation fits
    /// in the window.
    fn measure_scrollbar(&mut self, view_height: Pixels, content_height: Pixels) {
        let scroll_max = content_height - view_height;
        // Half a pixel of overflow is rounding, not something to scroll.
        if scroll_max <= px(0.5) || view_height <= px(0.0) {
            self.scrollbar = None;
            return;
        }

        let inset = px(theme::SCROLLBAR_INSET);
        // The bar runs to the bottom edge, where the window's rounded corner
        // would clip its lower end, so the track stops short of the arc.
        let foot = inset + px(theme::corner_clearance(theme::SCROLLBAR_INSET));
        let track_height = (view_height - inset - foot).max(px(0.0));
        // The thumb is as long a share of the track as the window is of the
        // document, down to a length that can still be caught with a pointer.
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

    fn render_scrollbar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let bar = self.scrollbar?;

        Some(
            div()
                .absolute()
                .top(bar.thumb_top)
                .right(px(theme::SCROLLBAR_INSET))
                .w(px(theme::SCROLLBAR_WIDTH))
                .h(bar.thumb_height)
                .child(
                    div()
                        .id("scrollbar-thumb")
                        .size_full()
                        .rounded(px(theme::SCROLLBAR_WIDTH / 2.0))
                        .bg(theme::scrollbar_thumb())
                        .cursor(CursorStyle::Arrow)
                        .hover(|style| style.bg(theme::scrollbar_thumb_hover()))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                                editor.scrollbar_grab = Some(event.position.y - bar.thumb_top);
                                cx.notify();
                            }),
                        ),
                ),
        )
    }

    fn render_result(
        &self,
        index: usize,
        output: Option<&LineOutput>,
        height: Pixels,
        budget: usize,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Div {
        let row = div()
            .h(height)
            .flex()
            .flex_row()
            .items_center()
            .justify_end();

        // An empty row keeps this column aligned with the input.
        let Some(output) = output else {
            return row;
        };

        let copied = self.copy_flash.filter(|flash| flash.result == index);

        let interaction = if self.open_result == Some(index) {
            result::Interaction::Open
        } else if self.pressed_result == Some(index) || copied.is_some_and(|flash| flash.lit()) {
            result::Interaction::Pressed
        } else if self.hovered_result == Some(index) {
            result::Interaction::Hovered
        } else {
            result::Interaction::Idle
        };

        row.child(
            div()
                .id(("result", index))
                .cursor(CursorStyle::Arrow)
                .on_hover(cx.listener(move |editor, hovered: &bool, _, cx| {
                    if *hovered {
                        editor.hovered_result = Some(index);
                    } else if editor.hovered_result == Some(index) {
                        editor.hovered_result = None;
                    }
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                        editor.pressed_result = Some(index);
                        // The press that opens a popover must not also read as
                        // the press outside one that closes it.
                        editor.press_handled = true;
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(move |editor, _: &ClickEvent, _, cx| {
                    editor.open_result = if editor.open_result == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                    cx.notify();
                }))
                .child(result::ResultElement::new(
                    output,
                    budget,
                    interaction,
                    metrics,
                    &self.fonts,
                    metrics.output_width - px(2.0 * theme::GUTTER),
                    copied.map_or(1.0, |flash| flash.scale()),
                )),
        )
    }

    /// The panel under an open result: the value it holds at full precision and
    /// a button to take it, or — for a line that did not work out — what went
    /// wrong and where.
    ///
    /// It is drawn at the top of the window rather than beside the result it
    /// belongs to, so that nothing clips it. Where it goes is arithmetic on what
    /// has already been measured: the result's row is a known height a known
    /// distance down the scrolled content, and its right edge is the column's,
    /// which is the window's less a gutter.
    fn render_popover(&self, layout: &Layout, cx: &mut Context<Self>) -> Option<Div> {
        let index = self.open_result?;
        let output = self.displayed.get(index)?.as_ref()?;

        // The result stands taller than the row holding it and is centred in it,
        // so it overhangs by the same amount top and bottom.
        let result_height = px(theme::ROOT_LINE_HEIGHT + 2.0 * theme::OUTPUT_PADDING_Y);
        let row_height = layout.lines.get(index)?.height;
        let row_top =
            px(theme::NAV_HEIGHT + theme::GUTTER) + layout.line_top(index) + self.scroll.offset().y;
        let bottom = row_top + (row_height - result_height) / 2.0 + result_height;

        let panel = below_right(bottom, px(theme::GUTTER))
            .flex()
            .flex_col()
            // A press inside the panel is not a press outside it.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, _| {
                    editor.press_handled = true;
                }),
            );

        Some(match output {
            LineOutput::Value { string, repeating } => {
                let copied = self
                    .copy_flash
                    .is_some_and(|flash| flash.result == index && flash.lit());

                panel
                    .child(
                        div()
                            .mb(px(theme::POPOVER_VALUE_GAP))
                            .child(result::ValueElement::new(
                                string,
                                *repeating,
                                &self.fonts,
                                px(theme::POPOVER_MAX_WIDTH - 2.0 * theme::POPOVER_PADDING),
                            )),
                    )
                    .child(
                        popover_button(
                            ("copy-result", index),
                            if copied { "Copied!" } else { "Copy" },
                        )
                        .on_click(cx.listener(
                            move |editor, _: &ClickEvent, _, cx| {
                                editor.copy_result(index, cx);
                            },
                        )),
                    )
            }
            LineOutput::Error {
                description,
                location,
                ..
            } => panel
                .child(
                    div()
                        .text_size(px(theme::FONT_SMALL))
                        .text_color(theme::popover_error_description())
                        .child(SharedString::from(description.clone().unwrap_or_default())),
                )
                .children(location.as_ref().map(|location| {
                    div()
                        .mt(px(theme::POPOVER_LOCATION_GAP))
                        .text_size(px(theme::FONT_TINY))
                        .text_color(theme::popover_error_location())
                        .child(SharedString::from(location.clone()))
                })),
        })
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// Convert between the platform's UTF-16 offsets and the text's UTF-8 offsets.
impl Editor {
    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text.offset_to_utf16(offset)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.text.range_from_utf16(range)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.text.range_to_utf16(range)
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.text.content().get(range)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(self.text.selection()),
            reversed: self.text.selection_reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.text
            .marked_range()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.text.unmark_text();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text.replace_text_in_range(range_utf16, new_text);
        self.text_changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text
            .replace_and_mark_text_in_range(range_utf16, new_text, new_selected_range_utf16);
        self.text_changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = layout.position_for_offset(range.start)?;
        let end = layout.position_for_offset(range.end)?;
        Some(Bounds::from_corners(
            element_bounds.origin + start,
            element_bounds.origin + point(end.x, end.y + layout.metrics.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.offset_for_position(point)))
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = Metrics::new(window, &self.fonts, self.output_fraction);
        let layout = Rc::new(Layout::new(
            self.text.content(),
            self.text.selection(),
            metrics,
            &self.fonts,
            window,
        ));
        self.layout = Some(layout.clone());

        // Before wrapping makes the column boundary matter, the separator only
        // appears while hovered or dragged.
        self.separator_fade.set(
            if layout.any_wrapped || self.separator_hovered || self.dragging_separator {
                1.0
            } else {
                0.0
            },
        );
        if !self.separator_fade.settled() {
            window.request_animation_frame();
        }

        let viewport = window.viewport_size();
        let min_height =
            (viewport.height - px(theme::NAV_HEIGHT) - px(2.0 * theme::GUTTER)).max(px(0.0));
        let budget = metrics.output_character_budget();

        // What the scrolling container holds, and how much of it shows. Both
        // follow from the window and the text without measuring anything: the
        // columns are a gutter's padding around the taller of the text and the
        // window, and the container is the window less the title strip.
        let view_height = viewport.height - px(theme::NAV_HEIGHT);
        let content_height = layout.content_height().max(min_height) + px(2.0 * theme::GUTTER);

        if std::mem::take(&mut self.autoscroll) {
            self.scroll_caret_into_view(&layout, view_height, content_height);
        }
        self.measure_scrollbar(view_height, content_height);

        // Which diagnostics are on show was settled when the text or the caret
        // last moved; drawing a frame does not reconsider it.
        let visible = |index: usize| -> Option<&LineOutput> {
            self.displayed.get(index).and_then(|output| output.as_ref())
        };

        let results: Vec<_> = layout
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                self.render_result(index, visible(index), line.height, budget, metrics, cx)
            })
            .collect();

        // What each diagnostic on show is pointing at, marked out in the text
        // itself. The engine counts a span in characters, as the parser handed
        // it over; the text is addressed in bytes.
        let error_spans: Vec<Option<Range<usize>>> = layout
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let Some(LineOutput::Error {
                    span: Some((start, end)),
                    ..
                }) = visible(index)
                else {
                    return None;
                };

                let text = &self.text.content()[line.start..line.start + line.len];
                let range = byte_offset(text, *start)..byte_offset(text, *end);
                (range.start < range.end).then_some(range)
            })
            .collect();

        let rulers: Vec<_> = layout
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let output = visible(index);
                let end_padding = output.map_or(px(0.0), |output| {
                    result::width(output, budget, metrics)
                        .min(metrics.output_width - px(2.0 * theme::GUTTER))
                });
                let active = metrics.rulers_visible() && output.is_some();

                let span = metrics.window_width
                    - px(2.0 * theme::GUTTER)
                    - line.ruler_start
                    - px(theme::RULER_GAP_LEFT)
                    - end_padding
                    - px(theme::RULER_GAP_RIGHT);

                div().h(line.height).flex().flex_row().items_center().child(
                    div()
                        .ml(line.ruler_start + px(theme::RULER_GAP_LEFT))
                        .w(span.max(px(0.0)))
                        .h(px(theme::RULER_THICKNESS))
                        .when(active, |this| this.bg(theme::ruler())),
                )
            })
            .collect();

        let popover = self.render_popover(&layout, cx);
        let toast = self.toast.render();

        // Both of these are read off the clock rather than driven by a timer, so
        // a frame is only worth drawing while one of them is still moving.
        if self.toast.animating() || self.copy_flash.is_some_and(|flash| flash.lit()) {
            window.request_animation_frame();
        }

        div()
            .size_full()
            .relative()
            .bg(theme::BACKGROUND)
            .text_color(theme::text())
            .text_size(px(theme::REM))
            .line_height(px(theme::ROOT_LINE_HEIGHT))
            .font_family(self.fonts.regular.family.clone())
            .font_weight(FontWeight::SEMIBOLD)
            .flex()
            .flex_col()
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            // Registered before anything inside it, and so run after all of
            // them: by the time this is reached, a press that belonged to the
            // popover or to the result holding it open has said so, and any
            // other press is one that lands outside and closes it.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                    if !std::mem::take(&mut editor.press_handled) {
                        editor.open_result = None;
                        cx.notify();
                    }
                }),
            )
            // The window's own title bar is drawn transparent, so this bar
            // stands in for it and keeps the traffic lights clear of the text.
            .child(self.toolbar.clone())
            .child(
                div()
                    .id("editor-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_row()
                            // Left at the flex default of `stretch`: the
                            // separator is a 1px anchor with nothing in the
                            // flow to give it height, and it has to run the
                            // full height of the row to be grabbable at all.
                            .w_full()
                            // Behind everything, and inset by the same 1rem the
                            // two columns are, so the rows line up.
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .p(px(theme::GUTTER))
                                    .children(rulers),
                            )
                            .child(
                                div()
                                    .key_context("SiffraEditor")
                                    .track_focus(&self.focus_handle)
                                    .w(metrics.input_width)
                                    .flex_shrink_0()
                                    .overflow_hidden()
                                    .p(px(theme::GUTTER))
                                    .cursor(CursorStyle::IBeam)
                                    .on_action(cx.listener(Self::backspace))
                                    .on_action(cx.listener(Self::delete))
                                    .on_action(cx.listener(Self::delete_to_line_start))
                                    .on_action(cx.listener(Self::left))
                                    .on_action(cx.listener(Self::right))
                                    .on_action(cx.listener(Self::up))
                                    .on_action(cx.listener(Self::down))
                                    .on_action(cx.listener(Self::word_left))
                                    .on_action(cx.listener(Self::word_right))
                                    .on_action(cx.listener(Self::select_word_left))
                                    .on_action(cx.listener(Self::select_word_right))
                                    .on_action(cx.listener(Self::delete_word_left))
                                    .on_action(cx.listener(Self::delete_word_right))
                                    .on_action(cx.listener(Self::select_left))
                                    .on_action(cx.listener(Self::select_right))
                                    .on_action(cx.listener(Self::select_up))
                                    .on_action(cx.listener(Self::select_down))
                                    .on_action(cx.listener(Self::select_all))
                                    .on_action(cx.listener(Self::home))
                                    .on_action(cx.listener(Self::end))
                                    .on_action(cx.listener(Self::select_home))
                                    .on_action(cx.listener(Self::select_end))
                                    .on_action(cx.listener(Self::document_start))
                                    .on_action(cx.listener(Self::document_end))
                                    .on_action(cx.listener(Self::newline))
                                    .on_action(cx.listener(Self::copy))
                                    .on_action(cx.listener(Self::cut))
                                    .on_action(cx.listener(Self::paste))
                                    .on_action(cx.listener(Self::undo))
                                    .on_action(cx.listener(Self::redo))
                                    .on_action(cx.listener(Self::show_character_palette))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(Self::on_mouse_down),
                                    )
                                    .child(InputElement::new(
                                        cx.entity(),
                                        layout.clone(),
                                        min_height,
                                        error_spans,
                                    )),
                            )
                            .child(self.render_separator(cx))
                            .child(
                                div()
                                    .w(metrics.output_width)
                                    .flex_shrink_0()
                                    .p(px(theme::GUTTER))
                                    .children(results),
                            ),
                    ),
            )
            // Over the content rather than beside it, so showing the bar does
            // not narrow the columns and re-wrap the text under it.
            .children(self.render_scrollbar(cx))
            // Above the columns, and outside the scrolling container so that
            // nothing clips either of them.
            .children(popover)
            .children(toast)
    }
}

/// Where a character index falls in a line, in bytes.
///
/// Error spans are counted in characters, because that is how the parser reads
/// its input; the text is addressed in bytes everywhere else.
fn byte_offset(text: &str, characters: usize) -> usize {
    text.char_indices()
        .nth(characters)
        .map_or(text.len(), |(offset, _)| offset)
}

impl Editor {
    fn render_separator(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let grab = px(theme::SEPARATOR_GRAB_PADDING);

        // A 1px anchor in the flow, with the grab area hung off it and shifted
        // half its own width left, so the rule sits exactly on the boundary.
        div()
            .w(px(theme::SEPARATOR_WIDTH))
            .flex_shrink_0()
            .relative()
            .child(
                div()
                    .id("separator")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    // Center the grab box on the 1px boundary.
                    .left(-(grab + px(theme::SEPARATOR_WIDTH / 2.0)))
                    .w(grab * 2.0 + px(theme::SEPARATOR_WIDTH))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .opacity(self.separator_fade.value())
                    .on_hover(cx.listener(|editor, hovered: &bool, _, cx| {
                        editor.separator_hovered = *hovered;
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                            editor.dragging_separator = true;
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .w(px(theme::SEPARATOR_WIDTH))
                            .h_full()
                            .bg(theme::separator()),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::carry_edit_times;

    fn lines(text: &str) -> Vec<String> {
        text.split('\n').map(str::to_string).collect()
    }

    /// `1` stands for a line last edited a while ago, `2` for one just typed.
    fn carry(previous: &str, times: &[u32], next: &str) -> Vec<Option<u32>> {
        let times: Vec<Option<u32>> = times.iter().map(|time| Some(*time)).collect();
        carry_edit_times(&lines(previous), &times, &lines(next), 2)
    }

    #[test]
    fn typing_on_one_line_leaves_its_neighbours_alone() {
        assert_eq!(
            carry("a\nb\nc", &[1, 1, 1], "a\nB\nc"),
            [Some(1), Some(2), Some(1)]
        );
    }

    #[test]
    fn a_change_that_changes_nothing_resets_no_timers() {
        assert_eq!(
            carry("a\nb\nc", &[1, 1, 1], "a\nb\nc"),
            [Some(1), Some(1), Some(1)]
        );
    }

    #[test]
    fn inserting_a_line_costs_only_that_line() {
        // Were the times indexed rather than matched from both ends, the `b`
        // that moved down a row would read as freshly typed, and its
        // diagnostic would go quiet for no reason.
        assert_eq!(
            carry("a\nb", &[1, 1], "a\n\nb"),
            [Some(1), Some(2), Some(1)]
        );
    }

    #[test]
    fn deleting_a_line_leaves_the_survivors_alone() {
        assert_eq!(carry("a\nb\nc", &[1, 1, 1], "a\nc"), [Some(1), Some(1)]);
    }

    #[test]
    fn typing_at_the_end_leaves_the_lines_above_alone() {
        assert_eq!(
            carry("a\nb", &[1, 1], "a\nb\nc"),
            [Some(1), Some(1), Some(2)]
        );
    }

    #[test]
    fn every_line_of_a_freshly_loaded_calculation_counts_as_typed() {
        assert_eq!(carry("", &[], "a\nb"), [Some(2), Some(2)]);
    }
}
