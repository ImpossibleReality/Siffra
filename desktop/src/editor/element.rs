//! The input column: the shaped lines, the caret, and the rules that mark
//! headings out in the gutter.
//!
//! Everything here is painted rather than composed out of divs, because the
//! text is one thing measured once — a line, its wrapping, and the caret's
//! place inside it all come from the same shaped layout, and drawing it as a
//! stack of elements would mean measuring it twice and hoping the two agreed.

use super::Editor;
use super::layout::{Layout, row_rects};
use crate::theme;
use gpui::{
    App, Bounds, Corners, Element, ElementId, ElementInputHandler, Entity, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Style, TextAlign, Window, point, px, quad,
    relative, size,
};
use std::ops::Range;
use std::rc::Rc;

pub struct InputElement {
    editor: Entity<Editor>,
    layout: Rc<Layout>,
    /// Minimum input height, so clicks below a short calculation still land in
    /// the text area.
    min_height: Pixels,
    /// For each line, the byte range its diagnostic points at, if it has one
    /// that is being shown. Held per line rather than as one range over the
    /// document because that is how the engine reports it, and because a span
    /// never crosses a line.
    errors: Vec<Option<Range<usize>>>,
}

impl InputElement {
    pub fn new(
        editor: Entity<Editor>,
        layout: Rc<Layout>,
        min_height: Pixels,
        errors: Vec<Option<Range<usize>>>,
    ) -> Self {
        Self {
            editor,
            layout,
            min_height,
            errors,
        }
    }
}

impl IntoElement for InputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for InputElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = self.layout.content_height().max(self.min_height).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Mouse handling lives on the wrapper div, which needs to know where the
        // text ended up to turn a click into an offset. Keeping the caret in
        // view is not done here: the scrolling container has already read its
        // offset by this point, so it is settled while the frame is laid out
        // instead — see `Editor::scroll_caret_into_view`.
        self.editor.update(cx, |editor, _| {
            editor.text_bounds = Some(bounds);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let focus_handle = editor.focus_handle.clone();
        let selection = editor.text.selection().clone();
        let caret_offset = editor.caret_offset();
        let caret_visible = editor.caret_visible;
        let metrics = self.layout.metrics;

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        let mut top = bounds.origin.y;
        for (index, line) in self.layout.lines.iter().enumerate() {
            let origin = point(bounds.origin.x, top);

            // A `#` line is a heading wherever it sits. Every line is set in the
            // same type — the input has to wrap identically whatever a line
            // means — so a heading is marked by weight, colour and a rule in the
            // gutter, none of which move the text.
            if line.is_header {
                let inset = metrics.font_size * theme::HEADER_RULE_INSET_EM;
                window.paint_quad(quad(
                    Bounds::new(
                        point(bounds.origin.x - px(theme::HEADER_RULE_OFFSET), top + inset),
                        size(
                            px(theme::HEADER_RULE_WIDTH),
                            (line.height - inset * 2.0).max(px(0.0)),
                        ),
                    ),
                    Corners::all(px(theme::HEADER_RULE_WIDTH / 2.0)),
                    theme::hsla(theme::header_rule()),
                    px(0.0),
                    gpui::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            }

            // Drawn to the height of the line box, which is what the selection
            // on a line of text comes out as, so a run of selected lines reads
            // as one unbroken column however many of them are blank.
            if line.blank_and_selected {
                window.paint_quad(quad(
                    Bounds::new(
                        origin,
                        size(px(theme::EMPTY_LINE_SELECTION_WIDTH), metrics.line_height),
                    ),
                    Corners::all(px(0.0)),
                    theme::hsla(theme::selection()),
                    px(0.0),
                    gpui::transparent_black(),
                    gpui::BorderStyle::default(),
                ));
            }

            // What the diagnostic beside this line is pointing at, washed over
            // and underscored. The span is an inline box, so it covers the
            // text's own height rather than the looser line box holding it, and
            // its rule is drawn just below that — under the glyphs, which are
            // painted over the lot of it.
            if let Some(error) = self.errors.get(index).and_then(Option::as_ref) {
                let top_of_text = metrics.half_leading();
                let height = metrics.ascent + metrics.descent;

                for rect in row_rects(&line.shaped, error.clone(), metrics.line_height) {
                    let corner = origin + rect.origin + point(px(0.0), top_of_text);

                    window.paint_quad(quad(
                        Bounds::new(
                            corner,
                            size(rect.width, height + px(theme::ERROR_UNDERLINE)),
                        ),
                        Corners::all(px(0.0)),
                        theme::hsla(theme::error_span_bg()),
                        px(0.0),
                        gpui::transparent_black(),
                        gpui::BorderStyle::default(),
                    ));
                    window.paint_quad(quad(
                        Bounds::new(
                            corner + point(px(0.0), height),
                            size(rect.width, px(theme::ERROR_UNDERLINE)),
                        ),
                        Corners::all(px(0.0)),
                        theme::hsla(theme::error_span_underline()),
                        px(0.0),
                        gpui::transparent_black(),
                        gpui::BorderStyle::default(),
                    ));
                }
            }

            line.shaped
                .paint_background(
                    origin,
                    metrics.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            line.shaped
                .paint(
                    origin,
                    metrics.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();

            top += line.height;
        }

        if let Some(placeholder) = &self.layout.placeholder {
            placeholder
                .paint(
                    bounds.origin,
                    metrics.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
        }

        // The caret is drawn to the height of the text rather than the height
        // of the line box, and only for a plain caret — a selection shows as
        // its own highlight and does not want a bar as well.
        if focus_handle.is_focused(window)
            && caret_visible
            && selection.is_empty()
            && let Some(position) = self.layout.position_for_offset(caret_offset)
        {
            window.paint_quad(quad(
                Bounds::new(
                    bounds.origin + point(position.x, position.y + metrics.half_leading()),
                    size(px(theme::CARET_WIDTH), metrics.ascent + metrics.descent),
                ),
                Corners::all(px(theme::CARET_WIDTH / 2.0)),
                theme::hsla(theme::caret()),
                px(0.0),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
        }
    }
}
