//! The window: one calculation open at a time, and everything that decides which
//! one that is.
//!
//! There is no save command. A calculation is written to the store shortly after
//! the typing on it stops, and again whenever it is about to be left — switching
//! away from it, closing the window, quitting. Naming a calculation is what
//! "saving" one means here, which is why ⌘S puts the caret in its name.

use crate::documents::{Document, DocumentStore, database_path};
use crate::editor::layout::Fonts;
use crate::editor::{ContentChanged, Editor};
use crate::help::{CloseHelp, HelpModal};
use crate::search::{SearchEvent, SearchModal};
use crate::toolbar::{Toolbar, ToolbarEvent};
use crate::updater;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Subscription, Task, Window, actions, div,
};
use std::time::Duration;

actions!(
    siffra,
    [
        NewDocument,
        NameDocument,
        ToggleSearch,
        ToggleHelp,
        ToggleAlwaysOnTop,
        CheckForUpdates,
        InstallUpdate
    ]
);

/// How long typing settles before the document is written to the store.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

pub struct Workspace {
    /// `None` when there is nowhere to keep documents, which leaves the session
    /// working but unsaved rather than refusing to open at all.
    store: Option<DocumentStore>,
    current: Option<Document>,
    editor: Entity<Editor>,
    toolbar: Entity<Toolbar>,
    help: Entity<HelpModal>,
    help_open: bool,
    search: Entity<SearchModal>,
    search_open: bool,
    /// Text typed since the last write, or `None` when the store is up to date.
    unsaved: Option<String>,
    save_timer: Option<Task<()>>,
    /// The verified update selected by a manual update check. It is only
    /// installed when the user selects the separate install command.
    pending_update: Option<cargo_packager_updater::Update>,
    update_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(fonts: Fonts, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mono_family = fonts.regular.family.clone();
        let toolbar = cx.new(Toolbar::new);
        let help = cx.new(|cx| HelpModal::new(mono_family.clone(), cx));
        let search = cx.new(|cx| SearchModal::new(mono_family, cx));
        let editor = cx.new(|cx| Editor::new("", fonts, toolbar.clone(), cx));

        let subscriptions = vec![
            cx.subscribe_in(&toolbar, window, Self::on_toolbar_event),
            cx.subscribe_in(&search, window, Self::on_search_event),
            cx.subscribe(&help, |workspace, _, _: &CloseHelp, cx| {
                workspace.set_help_open(false, cx);
            }),
            cx.subscribe(&editor, |workspace, _, _: &ContentChanged, cx| {
                workspace.content_changed(cx);
            }),
            // Flush when activation ends because no later keystroke may arrive.
            cx.observe_window_activation(window, |workspace, window, cx| {
                if !window.is_window_active() {
                    workspace.flush_save(cx);
                }
            }),
        ];
        // Flush before allowing the window to close.
        let closing = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            closing
                .update(cx, |workspace, cx| workspace.flush_save(cx))
                .ok();
            true
        });

        let store = database_path().and_then(|path| DocumentStore::open(&path).ok());
        let mut workspace = Self {
            store,
            current: None,
            editor,
            toolbar,
            help,
            help_open: false,
            search,
            search_open: false,
            unsaved: None,
            save_timer: None,
            pending_update: None,
            update_task: None,
            _subscriptions: subscriptions,
        };

        if let Some(document) = workspace
            .store
            .as_ref()
            .and_then(|store| store.open_latest().ok().flatten())
        {
            workspace.show(document, cx);
        }
        workspace
    }

    pub fn editor_focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle(cx)
    }

    /// Writes what is pending, for the moments outside the window's own life
    /// that have to force one: quitting, and closing from the menu.
    pub fn flush_pending_save(&mut self, cx: &mut Context<Self>) {
        self.flush_save(cx);
    }

    /// Switches the editor to `document`, writing any pending edits to the one
    /// being left first.
    fn show(&mut self, document: Document, cx: &mut Context<Self>) {
        self.flush_save(cx);
        self.editor.update(cx, |editor, cx| {
            editor.set_document(&document.content, document.cursor, cx);
        });
        self.toolbar.update(cx, |toolbar, cx| {
            toolbar.set_title(document.title.clone(), cx);
        });
        self.current = Some(document);
        self.search_open = false;
        cx.notify();
    }

    fn open_document(&mut self, id: u64, cx: &mut Context<Self>) {
        // Flush before reading the next document so an autosave cannot race it.
        self.flush_save(cx);
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let Ok(Some(document)) = store.get(id) else {
            return;
        };
        store.set_last_opened(id).ok();
        self.show(document, cx);
    }

    /// Creates a blank calculation and puts the caret at the end of it, ready to
    /// be typed into. Shared by ⌘N and the bar's button.
    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_save(cx);
        self.show_blank(cx);
        self.focus_editor_end(window, cx);
    }

    /// Starts an in-memory calculation. It becomes a saved document only after
    /// the editor contains non-whitespace content.
    fn show_blank(&mut self, cx: &mut Context<Self>) {
        self.discard_pending_save();
        self.current = None;
        self.editor.update(cx, |editor, cx| {
            editor.set_document("", 0, cx);
        });
        self.toolbar.update(cx, |toolbar, cx| {
            toolbar.set_title("", cx);
        });
        self.search_open = false;
        cx.notify();
    }

    fn rename_document(&mut self, title: String, cx: &mut Context<Self>) {
        let Some((store, current)) = self.store.as_ref().zip(self.current.as_ref()) else {
            return;
        };
        let Ok(Some(renamed)) = store.rename(current.id, &title) else {
            return;
        };
        self.toolbar.update(cx, |toolbar, cx| {
            toolbar.set_title(renamed.title.clone(), cx);
        });
        self.current = Some(renamed);
        cx.notify();
    }

    /// Deletes a calculation. Deleting the open one leaves nothing to show, so a
    /// fresh blank calculation takes its place and the search modal closes.
    fn delete_document(&mut self, id: u64, cx: &mut Context<Self>) {
        let open = self
            .current
            .as_ref()
            .is_some_and(|current| current.id == id);
        let deleted = self
            .store
            .as_ref()
            .is_some_and(|store| store.delete(id).is_ok());
        if !deleted {
            return;
        }

        if open {
            // Saving recreates a missing record, so the open document has to be
            // let go of entirely: both the edits waiting to be written and the
            // caret, which on its own would be enough to write the record back.
            self.show_blank(cx);
        } else {
            self.refresh_search(cx);
        }
        cx.notify();
    }

    fn content_changed(&mut self, cx: &mut Context<Self>) {
        self.unsaved = Some(self.editor.read(cx).content().to_string());
        self.save_timer = Some(cx.spawn(async move |workspace, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            workspace
                .update(cx, |workspace, cx| workspace.flush_save(cx))
                .ok();
        }));
    }

    /// Writes what is pending, if anything is. The caret travels with the text,
    /// so a calculation reopens where it was left — including when only the caret
    /// moved.
    fn flush_save(&mut self, cx: &mut Context<Self>) {
        self.save_timer = None;
        let Some(store) = self.store.as_ref() else {
            return;
        };

        let cursor = self.editor.read(cx).cursor();
        if self.unsaved.is_none()
            && self
                .current
                .as_ref()
                .is_none_or(|current| cursor == current.cursor)
        {
            return;
        }

        let content = self.unsaved.take().unwrap_or_else(|| {
            self.current
                .as_ref()
                .map_or_else(String::new, |current| current.content.clone())
        });

        if content.trim().is_empty() {
            if let Some(current) = self.current.take() {
                if store.delete(current.id).is_err() {
                    self.current = Some(current);
                    self.unsaved = Some(content);
                    return;
                }
            }
            self.toolbar.update(cx, |toolbar, cx| {
                toolbar.set_title("", cx);
            });
            return;
        }

        let was_new = self.current.is_none();
        let saved = match self.current.as_ref() {
            Some(current) => store.save(current.id, &content, cursor),
            None => store.create(&content).and_then(|created| {
                if cursor == 0 {
                    Ok(created)
                } else {
                    store.save(created.id, &content, cursor)
                }
            }),
        };
        if let Ok(saved) = saved {
            store.set_last_opened(saved.id).ok();
            if was_new {
                self.toolbar.update(cx, |toolbar, cx| {
                    toolbar.set_title(saved.title.clone(), cx);
                });
            }
            self.current = Some(saved);
        }
    }

    /// Drops edits that have not been written yet — used when their document is
    /// deleted, since saving recreates a missing record.
    fn discard_pending_save(&mut self) {
        self.save_timer = None;
        self.unsaved = None;
    }

    fn set_help_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.help_open = open;
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.set_help_open(open, cx));
        if open {
            self.help.update(cx, HelpModal::open);
        }
        cx.notify();
    }

    /// Every saved calculation, with whatever is being typed written first so the
    /// list shows it.
    fn saved_documents(&mut self, cx: &mut Context<Self>) -> Vec<Document> {
        self.flush_save(cx);
        self.store
            .as_ref()
            .and_then(|store| store.list().ok())
            .unwrap_or_default()
    }

    fn open_search(&mut self, cx: &mut Context<Self>) {
        let documents = self.saved_documents(cx);
        self.search_open = true;
        self.search
            .update(cx, |search, cx| search.open(documents, cx));
        cx.notify();
    }

    fn close_search(&mut self, cx: &mut Context<Self>) {
        if self.search_open {
            self.search_open = false;
            cx.notify();
        }
    }

    /// Hands the modal a list it can still trust, after something has changed
    /// under it.
    fn refresh_search(&mut self, cx: &mut Context<Self>) {
        if !self.search_open {
            return;
        }
        let documents = self.saved_documents(cx);
        self.search
            .update(cx, |search, cx| search.set_documents(documents, cx));
    }

    /// Hands the keyboard back to the calculation without disturbing its caret —
    /// which for one just opened is where it was left when it was last closed.
    fn focus_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, _| editor.focus(window));
    }

    /// Hands the keyboard back with the caret at the end, which is where typing
    /// carries on from after a calculation is created or named.
    fn focus_editor_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.focus_end(window, cx));
    }

    fn on_toolbar_event(
        &mut self,
        _: &Entity<Toolbar>,
        event: &ToolbarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ToolbarEvent::NewDocument => self.new_document(window, cx),
            ToolbarEvent::OpenSearch => self.open_search(cx),
            ToolbarEvent::ToggleHelp => {
                let open = !self.help_open;
                self.set_help_open(open, cx);
            }
            ToolbarEvent::Rename(title) => self.rename_document(title.clone(), cx),
            ToolbarEvent::NameEditingFinished => self.focus_editor_end(window, cx),
        }
    }

    fn on_search_event(
        &mut self,
        _: &Entity<SearchModal>,
        event: &SearchEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SearchEvent::Close => {
                self.close_search(cx);
                self.focus_editor(window, cx);
            }
            SearchEvent::Open(id) => {
                let id = *id;
                self.close_search(cx);
                self.open_document(id, cx);
                self.focus_editor(window, cx);
            }
            SearchEvent::Delete(id) => self.delete_document(*id, cx),
        }
    }

    fn toggle_search(&mut self, _: &ToggleSearch, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_open {
            self.close_search(cx);
            self.focus_editor(window, cx);
        } else {
            self.open_search(cx);
        }
    }

    /// There is nothing else to save — edits are written as they are typed — so
    /// ⌘S names the calculation instead.
    fn name_document(&mut self, _: &NameDocument, window: &mut Window, cx: &mut Context<Self>) {
        self.close_search(cx);
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.start_editing(window, cx));
    }

    fn create_document(&mut self, _: &NewDocument, window: &mut Window, cx: &mut Context<Self>) {
        self.close_search(cx);
        self.new_document(window, cx);
    }

    /// The Help menu's way to the panel the bar's `?` opens.
    fn toggle_help(&mut self, _: &ToggleHelp, _: &mut Window, cx: &mut Context<Self>) {
        let open = !self.help_open;
        self.set_help_open(open, cx);
    }

    /// The View menu's way to the pin, which is the toolbar's to hold: the
    /// button has to keep showing whether the window is being held up.
    fn toggle_always_on_top(
        &mut self,
        _: &ToggleAlwaysOnTop,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.toggle_pin(window, cx));
    }

    fn toast(&self, message: impl Into<gpui::SharedString>, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.show_toast(message, cx));
    }

    /// Fetches the signed release manifest off the UI thread. An available
    /// update remains pending until the user chooses "Install Update…".
    fn check_for_updates(&mut self, _: &CheckForUpdates, _: &mut Window, cx: &mut Context<Self>) {
        if self.update_task.is_some() {
            return;
        }
        self.toast("Checking for updates…", cx);
        let work = cx.background_spawn(async { updater::check() });
        self.update_task = Some(cx.spawn(async move |workspace, cx| {
            let result = work.await;
            workspace
                .update(cx, |workspace, cx| {
                    workspace.update_task = None;
                    match result {
                        Ok(Some(update)) => {
                            let message = updater::summary(&update);
                            workspace.pending_update = Some(update);
                            workspace.toast(message, cx);
                        }
                        Ok(None) => workspace.toast("Siffra is up to date.", cx),
                        Err(error) => {
                            workspace.toast(format!("Could not check for updates: {error}"), cx)
                        }
                    }
                })
                .ok();
        }));
    }

    /// Downloads and applies the already verified update. `cargo-packager-
    /// updater` verifies the signature again immediately before installation.
    fn install_update(&mut self, _: &InstallUpdate, _: &mut Window, cx: &mut Context<Self>) {
        let Some(update) = self.pending_update.take() else {
            self.toast("Check for updates first.", cx);
            return;
        };
        self.toast(format!("Installing Siffra {}…", update.version), cx);
        let work = cx.background_spawn(async move { updater::install(&update) });
        self.update_task = Some(cx.spawn(async move |workspace, cx| {
            let result = work.await;
            workspace
                .update(cx, |workspace, cx| {
                    workspace.update_task = None;
                    match result {
                        Ok(()) => {
                            workspace.toast("Update installed. Restart Siffra to use it.", cx)
                        }
                        Err(error) => {
                            workspace.toast(format!("Could not install the update: {error}"), cx)
                        }
                    }
                })
                .ok();
        }));
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .on_action(cx.listener(Self::toggle_search))
            .on_action(cx.listener(Self::name_document))
            .on_action(cx.listener(Self::create_document))
            .on_action(cx.listener(Self::toggle_help))
            .on_action(cx.listener(Self::toggle_always_on_top))
            .on_action(cx.listener(Self::check_for_updates))
            .on_action(cx.listener(Self::install_update))
            .child(self.editor.clone())
            .children(self.help_open.then(|| self.help.clone()))
            .children(self.search_open.then(|| self.search.clone()))
    }
}

impl Focusable for Workspace {
    /// The keyboard belongs to the calculation unless a modal has taken it.
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.search_open {
            self.search.read(cx).focus_handle(cx)
        } else if self.help_open {
            self.help.read(cx).focus_handle(cx)
        } else {
            self.editor.read(cx).focus_handle(cx)
        }
    }
}
