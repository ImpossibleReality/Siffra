use gpui::{Hsla, Pixels, Rgba, px, rgb, rgba};

pub const REM: f32 = 15.0;

pub const ROOT_LINE_HEIGHT: f32 = 24.0;

pub const NAV_HEIGHT: f32 = 2.1 * REM;

/// Inset that keeps input, rulers, and results on one row.
pub const GUTTER: f32 = REM;

pub const LINE_HEIGHT_RATIO: f32 = 1.8;

pub const OUTPUT_PADDING_X: f32 = 0.5 * REM;
pub const OUTPUT_PADDING_Y: f32 = 0.2 * REM;
pub const OUTPUT_RADIUS: f32 = 0.3 * REM;

/// Preferred output width and its lower and upper bounds.
pub const OUTPUT_FRACTION: f32 = 0.40;
pub const OUTPUT_MIN_WIDTH: f32 = 6.0 * REM;
pub const OUTPUT_MAX_INSET: f32 = 8.0 * REM;

/// Ruler thickness and the gaps at either side.
pub const RULER_THICKNESS: f32 = 2.0;
pub const RULER_GAP_LEFT: f32 = 10.0;
pub const RULER_GAP_RIGHT: f32 = 5.0;

pub const RULER_MIN_WINDOW_WIDTH: f32 = 500.0;

/// Heading rule geometry in the gutter.
pub const HEADER_RULE_OFFSET: f32 = 0.55 * REM;
pub const HEADER_RULE_WIDTH: f32 = 2.0;
/// In `em`, so it is resolved against the editor's font size, not the rem size.
pub const HEADER_RULE_INSET_EM: f32 = 0.3;

pub const CARET_WIDTH: f32 = 2.0;
/// Caret blink period in seconds.
pub const CARET_BLINK_PERIOD: f32 = 1.06;

/// Separator line and its additional hit-area padding.
pub const SEPARATOR_WIDTH: f32 = 1.0;
pub const SEPARATOR_GRAB_PADDING: f32 = 0.5 * REM;
/// Separator fade duration in seconds.
pub const SEPARATOR_FADE: f32 = 0.2;

pub const SCROLLBAR_WIDTH: f32 = 6.0;
pub const SCROLLBAR_INSET: f32 = 4.0;
pub const SCROLLBAR_MIN_THUMB: f32 = 28.0;

/// The radius the platform rounds the window's corners to -- used for scrollbar padding
pub const WINDOW_CORNER_RADIUS: f32 = 24.0;

pub fn corner_clearance(inset: f32) -> f32 {
    let radius = WINDOW_CORNER_RADIUS;
    let across = (radius - inset).clamp(0.0, radius);
    radius - (radius * radius - across * across).sqrt()
}

/// The sliver of selection drawn on a selected blank line
pub const EMPTY_LINE_SELECTION_WIDTH: f32 = 3.0;

pub const BACKGROUND: Rgba = Rgba {
    r: 33.0 / 255.0,
    g: 35.0 / 255.0,
    b: 41.0 / 255.0,
    a: 1.0,
};

pub fn text() -> Rgba {
    rgb(0xcdd4d7)
}

pub fn placeholder() -> Rgba {
    rgb(0x4b5263)
}

pub fn selection() -> Rgba {
    rgba(0x90bee340)
}

pub fn caret() -> Rgba {
    rgb(0xcdd4d7)
}

pub fn number() -> Rgba {
    rgb(0xdec4ab)
}

pub fn chemical() -> Rgba {
    rgb(0xc9a0dc)
}

pub fn unit() -> Rgba {
    rgb(0xe5c07b)
}

pub fn variable() -> Rgba {
    rgb(0xcdd4d7)
}

pub fn operation() -> Rgba {
    rgb(0x90bee3)
}

pub fn comment() -> Rgba {
    rgb(0x727b8c)
}

pub fn header() -> Rgba {
    rgb(0xe6ebee)
}

/// The rule drawn in the gutter beside a heading.
pub fn header_rule() -> Rgba {
    rgb(0x98c379)
}

pub fn ruler() -> Rgba {
    rgba(0xffffff08)
}

pub fn separator() -> Rgba {
    rgb(0x4b5263)
}

pub fn scrollbar_thumb() -> Rgba {
    rgba(0xcdd4d733)
}

pub fn scrollbar_thumb_hover() -> Rgba {
    rgba(0xcdd4d766)
}

pub fn result_value() -> Rgba {
    rgb(0x98c379)
}

pub fn result_error() -> Rgba {
    rgb(0xea6868)
}

pub fn result_value_hover_bg() -> Rgba {
    rgb(0x98c379)
}

pub fn result_hover_text() -> Rgba {
    rgb(0x282c34)
}

pub fn result_error_hover_bg() -> Rgba {
    rgb(0xb93737)
}

pub fn result_error_hover_text() -> Rgba {
    rgb(0xf3d6d6)
}

pub fn result_value_active_bg() -> Rgba {
    rgb(0x719657)
}

pub fn result_error_active_bg() -> Rgba {
    rgb(0x981e1e)
}

/// Open results keep this background while hovered or pressed; open errors use
/// their error-specific colour.
pub fn result_value_open_bg() -> Rgba {
    rgb(0x83a969)
}

pub const COPY_SCALE: f32 = 0.94;
pub const COPY_SQUISH: f32 = 0.14;
pub const COPY_EASE: (f32, f32, f32, f32) = (0.2, 0.8, 0.2, 1.0);
/// Duration of the copied-state flash in seconds.
pub const COPY_FLASH: f32 = 0.3;

pub const ERROR_UNDERLINE: f32 = 2.0;

pub fn error_span_bg() -> Rgba {
    rgba(0xff282812)
}

pub fn error_span_underline() -> Rgba {
    rgba(0xff2828cc)
}

pub const POPOVER_GAP: f32 = 0.3 * REM;
pub const POPOVER_MIN_WIDTH: f32 = 8.0 * REM;
pub const POPOVER_MAX_WIDTH: f32 = 20.0 * REM;
pub const POPOVER_PADDING: f32 = 0.6 * REM;
pub const POPOVER_RADIUS: f32 = 0.4 * REM;
pub const POPOVER_VALUE_GAP: f32 = 0.5 * REM;
pub const POPOVER_BUTTON_PADDING_X: f32 = REM;
pub const POPOVER_BUTTON_PADDING_Y: f32 = 0.4 * REM;
pub const POPOVER_BUTTON_RADIUS: f32 = 0.3 * REM;
pub const POPOVER_LOCATION_GAP: f32 = 0.4 * REM;

pub const FONT_SMALL: f32 = 0.9 * REM;
pub const FONT_TINY: f32 = 0.7 * REM;

pub fn popover_bg() -> Rgba {
    rgb(0x272b33)
}

pub fn popover_border() -> Rgba {
    rgb(0x383d46)
}

pub fn popover_shadow() -> Rgba {
    rgba(0x00000059)
}

pub fn popover_value() -> Rgba {
    rgb(0xcdd4d7)
}

pub fn popover_button_bg() -> Rgba {
    rgb(0x383d46)
}

pub fn popover_button_text() -> Rgba {
    rgb(0xa9aeb9)
}

pub fn popover_button_hover_bg() -> Rgba {
    rgb(0x454b55)
}

pub fn popover_button_hover_text() -> Rgba {
    rgb(0xffffff)
}

pub fn popover_error_description() -> Rgba {
    rgb(0xf0c8c8)
}

pub fn popover_error_location() -> Rgba {
    rgb(0x8a9196)
}

pub const TOAST_BOTTOM: f32 = REM;
pub const TOAST_RISE: f32 = 4.0;
pub const TOAST_FADE: f32 = 0.14;
pub const TOAST_HOLD: f32 = 1.2;

pub const TOAST_PADDING_X: f32 = 0.58 * REM;
pub const TOAST_PADDING_Y: f32 = 0.28 * REM;
pub const TOAST_RADIUS: f32 = 999.0;
pub const TOAST_FONT: f32 = 0.72 * REM;

pub fn toast_bg() -> Rgba {
    rgba(0x272b33d1)
}

pub fn toast_border() -> Rgba {
    rgba(0x98c37929)
}

pub fn toast_text() -> Rgba {
    rgba(0xcdd4d7c7)
}

pub fn toast_shadow() -> Rgba {
    rgba(0x0000002e)
}

pub fn toolbar_bg() -> Rgba {
    BACKGROUND
}

/// macOS draws its window buttons 14pt square. Centring them in a bar this tall
/// is the whole of what the position below is for.
pub const TRAFFIC_LIGHT_SIZE: f32 = 14.0;
/// Where macOS puts the close button by default, kept as it is so the buttons
/// stay where the platform's own windows have them.
pub const TRAFFIC_LIGHT_LEFT: f32 = 9.0;

pub fn traffic_light_top() -> f32 {
    (NAV_HEIGHT - TRAFFIC_LIGHT_SIZE) / 2.0
}

pub const TITLE_FONT: f32 = 0.8 * REM;
pub const TITLE_LINE_HEIGHT: f32 = REM;
pub const TITLE_PADDING_X: f32 = 0.55 * REM;
pub const TITLE_PADDING_Y: f32 = 0.25 * REM;
pub const TITLE_RADIUS: f32 = 0.3 * REM;
pub const TITLE_MIN_WIDTH: f32 = 3.0 * REM;
/// Maximum title width and the side reserve needed by the controls.
pub const TITLE_MAX_WIDTH: f32 = 28.0 * REM;
pub const TITLE_SIDE_RESERVE: f32 = 13.0 * REM;

pub fn title_text() -> Rgba {
    rgb(0x8a9196)
}

pub fn title_placeholder() -> Rgba {
    rgb(0x545b66)
}

pub fn title_hover_text() -> Rgba {
    rgb(0xcdd4d7)
}

pub fn title_editing_bg() -> Rgba {
    rgba(0xffffff17)
}

pub fn title_editing_text() -> Rgba {
    rgb(0xe6ebee)
}

pub fn title_selection() -> Rgba {
    rgb(0x3f5a75)
}

pub fn title_selection_text() -> Rgba {
    rgb(0xf2f6f8)
}

pub fn toolbar_hover_bg() -> Rgba {
    rgba(0xffffff0f)
}

pub const ICON_BUTTON_SIZE: f32 = 1.5 * REM;
pub const ICON_BUTTON_RADIUS: f32 = 0.3 * REM;
pub const ICON_SIZE: f32 = 15.0;
pub const TOOLBAR_MENU_PADDING: f32 = 0.3 * REM;

pub fn toolbar_icon() -> Rgba {
    rgb(0x8a9196)
}

pub fn toolbar_icon_hover() -> Rgba {
    rgb(0xcdd4d7)
}

pub const TOGGLE_PADDING: f32 = 0.2 * REM;
pub const TOGGLE_RADIUS: f32 = 0.25 * REM;
pub const TOGGLE_ICON_SIZE: f32 = 14.0;

pub fn toggle_icon() -> Rgba {
    rgb(0x98a1a4)
}

pub fn toggle_on_icon() -> Rgba {
    rgb(0xffffff)
}

pub fn toggle_on_bg() -> Rgba {
    rgba(0xffffff1a)
}

pub fn toggle_active_bg() -> Rgba {
    rgba(0xffffff33)
}

/// Scales the type size with window width and rounds it to a tenth of a rem.
pub fn font_size_for_window(window_width: Pixels) -> Pixels {
    let rems = (0.9 + (f32::from(window_width) - 300.0) / 800.0).clamp(0.9, 1.1);
    px((rems * 10.0).round() / 10.0 * REM)
}

pub fn hsla(color: Rgba) -> Hsla {
    color.into()
}
