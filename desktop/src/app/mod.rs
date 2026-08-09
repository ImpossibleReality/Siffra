pub mod keymap;
pub mod menus;

use crate::editor::layout::Fonts;
use crate::platform;
use crate::ui::assets::Assets;
use crate::ui::theme;
use crate::workspace::Workspace;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
    point, px, size,
};

const MONO_FAMILIES: &[&str] = &[
    "Roboto Mono",
    "SF Mono",
    "Menlo",
    "Cascadia Mono",
    "Consolas",
    "DejaVu Sans Mono",
];

fn mono_family(cx: &App) -> &'static str {
    let available = cx.text_system().all_font_names();
    MONO_FAMILIES
        .iter()
        .copied()
        .find(|family| available.iter().any(|name| name == family))
        .unwrap_or("monospace")
}

fn key_bindings() -> Vec<KeyBinding> {
    keymap::bindings()
}

pub fn run() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        cx.bind_keys(key_bindings());
        menus::register_actions(cx);
        cx.set_menus(menus::app_menus());
        #[cfg(target_os = "macos")]
        platform::macos::detach_native_help_menu();

        cx.on_app_quit(|cx: &mut App| {
            for window in cx.windows() {
                if let Some(window) = window.downcast::<Workspace>() {
                    window
                        .update(cx, |workspace, _, cx| workspace.flush_pending_save(cx))
                        .ok();
                }
            }
            async {}
        })
        .detach();

        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let fonts = Fonts::new(mono_family(cx));
        let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Siffra".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(
                            px(theme::TRAFFIC_LIGHT_LEFT),
                            px(theme::traffic_light_top()),
                        )),
                    }),
                    window_min_size: Some(size(px(320.0), px(240.0))),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_rem_size(px(theme::REM));
                    cx.new(|cx| Workspace::new(fonts, window, cx))
                },
            )
            .expect("opening the window");

        window
            .update(cx, |workspace, window, cx| {
                window.focus(&workspace.editor_focus_handle(cx));
                cx.activate(true);
            })
            .ok();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{KeyContext, Keymap, Keystroke};

    fn contexts(names: &[&str]) -> Vec<KeyContext> {
        names
            .iter()
            .map(|name| {
                let mut context = KeyContext::new_with_defaults();
                context.add(name.to_string());
                context
            })
            .collect()
    }

    fn action_for(keystroke: &str, stack: &[&str]) -> Option<&'static str> {
        let keymap = Keymap::new(key_bindings());
        let keystroke = Keystroke::parse(keystroke).expect("a parseable keystroke");
        let (bindings, _) = keymap.bindings_for_input(&[keystroke], &contexts(stack));
        bindings.first().map(|binding| binding.action().name())
    }

    #[test]
    fn the_same_editing_keys_reach_both_the_calculation_and_a_field() {
        for context in ["SiffraEditor", "SiffraInput"] {
            assert_eq!(action_for("cmd-c", &[context]), Some("siffra::Copy"));
            assert_eq!(action_for("cmd-a", &[context]), Some("siffra::SelectAll"));
            assert_eq!(action_for("alt-left", &[context]), Some("siffra::WordLeft"));
        }
    }

    #[test]
    fn neither_view_answers_for_the_other() {
        assert_eq!(action_for("enter", &["SiffraInput"]), None);
        assert_eq!(action_for("shift-cmd-left", &["SiffraEditor"]), None);
        assert_eq!(
            action_for("enter", &["SiffraEditor"]),
            Some("siffra::Newline")
        );
        assert_eq!(
            action_for("shift-cmd-left", &["SiffraInput"]),
            Some("siffra::SelectHome")
        );
    }

    #[test]
    fn the_search_modal_keeps_its_own_delete() {
        assert_eq!(
            action_for("cmd-backspace", &["SiffraInput"]),
            Some("siffra::DeleteToLineStart")
        );
        assert_eq!(
            action_for("cmd-backspace", &["SiffraSearch", "SiffraInput"]),
            None
        );
    }

    #[test]
    fn the_windows_own_shortcuts_reach_through_a_field() {
        assert_eq!(
            action_for("cmd-k", &["SiffraInput"]),
            Some("siffra::ToggleSearch")
        );
        assert_eq!(
            action_for("cmd-?", &["SiffraInput"]),
            Some("siffra::ToggleHelp")
        );
        assert_eq!(
            action_for("cmd-p", &["SiffraInput"]),
            Some("siffra::ToggleAlwaysOnTop")
        );
        assert_eq!(
            action_for("cmd-w", &["SiffraInput"]),
            Some("siffra::CloseWindow")
        );
    }
}
