use super::Editor;
use super::layout::{Layout, row_rects};
use crate::ui::theme;
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
    /// Minimum height keeps clicks below short documents in the text area.
    min_height: Pixels,
    /// Per-line byte ranges for visible diagnostics.
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
