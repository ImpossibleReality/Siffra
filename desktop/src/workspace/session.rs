//! The workspace session: one calculation open at a time, and everything that decides which
//! one that is.
//!
//! There is no save command. A calculation is written to the store shortly after
//! the typing on it stops, and again whenever it is about to be left — switching
//! away from it, closing the window, quitting. Naming a calculation is what
//! "saving" one means here, which is why ⌘S puts the caret in its name.
//!
//! What is kept, and where, follows from what a calculation has. Content is
//! written to the store. An empty one that has been named is held in memory, so
//! it can still be switched back to, but there is nothing worth writing. An
//! empty one that has never been named is nothing at all: it goes the moment it
//! is left.

use super::toolbar::{Toolbar, ToolbarEvent};
use crate::documents::{Document, DocumentStore, database_path, now_millis};
use crate::editor::layout::Fonts;
use crate::editor::{ContentChanged, Editor};
use crate::help::{CloseHelp, HelpModal};
use crate::search::{SearchEvent, SearchModal};
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
    /// The calculation on screen. There is always one, even when it is empty and
    /// nothing is keeping it yet.
    current: Document,
    /// Whether the open calculation has a record in the store, which is what
    /// decides whether emptying it has anything to clean up.
    current_on_disk: bool,
    /// Named calculations with nothing in them. There is nothing to write, but
    /// the name is the user's, so they remain switchable to for the session.
    remembered: Vec<Document>,
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
        let editor = cx.new(|cx| Editor::new("", fonts, cx));

        let subscriptions = vec![
            cx.subscribe_in(&toolbar, window, Self::on_toolbar_event),
            cx.subscribe_in(&search, window, Self::on_search_event),
            cx.subscribe_in(&help, window, |workspace, _, _: &CloseHelp, window, cx| {
                workspace.set_help_open(false, window, cx);
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
            // A focused element can leave without anything taking its place: a
            // modal closes, or a rename is ended by a click somewhere that
            // takes no focus of its own. What is left is not merely a missing
            // caret. An action is dispatched along the path of what has focus,
            // so an empty path leaves the window's own shortcuts — and the menu
            // items that stand for them — with nowhere to be delivered, which
            // is why they stop working and grey out. Whatever should hold the
            // keyboard takes it back here, so nothing in the window is ever
            // left focusless for longer than the frame it happened in.
            cx.on_focus_lost(window, |workspace, window, cx| {
                workspace.focus_frontmost(window, cx);
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
            current: Document::new(0, now_millis()),
            current_on_disk: false,
            remembered: Vec::new(),
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

        // Siffra opens on a blank calculation rather than the last one edited.
        workspace.show_blank(cx);
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

    /// Puts `document` on screen. Whatever was open is left to the caller, which
    /// has either written it or deliberately let it go. `on_disk` says whether
    /// the store already holds the one arriving.
    fn open_in_editor(&mut self, document: Document, on_disk: bool, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, cx| {
            editor.set_document(&document.content, document.cursor, cx);
        });
        self.toolbar.update(cx, |toolbar, cx| {
            toolbar.set_title(document.display_title(), cx);
        });
        self.current = document;
        self.current_on_disk = on_disk;
        self.search_open = false;
        cx.notify();
    }

    fn open_document(&mut self, id: u64, cx: &mut Context<Self>) {
        // Flush before reading the next document so an autosave cannot race it.
        self.flush_save(cx);
        if let Some(remembered) = self
            .remembered
            .iter()
            .find(|document| document.id == id)
            .cloned()
        {
            self.open_in_editor(remembered, false, cx);
        } else if let Some(store) = self.store.as_ref()
            && let Ok(Some(document)) = store.get(id)
        {
            self.open_in_editor(document, true, cx);
        }
    }

    /// Creates a blank calculation and puts the caret at the end of it, ready to
    /// be typed into. Shared by ⌘N and the bar's button.
    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_save(cx);
        self.show_blank(cx);
        self.focus_editor_end(window, cx);
    }

    /// Starts an empty, unnamed calculation. Nothing is keeping it: it reaches
    /// the store once it has content, and memory once it has a name.
    ///
    /// Pending edits are dropped rather than written, so this can also stand in
    /// for a calculation that has just been deleted.
    fn show_blank(&mut self, cx: &mut Context<Self>) {
        let created_at = now_millis();
        let document = Document::new(self.unused_id(created_at), created_at);
        self.discard_pending_save();
        self.open_in_editor(document, false, cx);
    }

    /// An id nothing else is using — neither a stored calculation nor one being
    /// held in memory.
    fn unused_id(&self, created_at: i64) -> u64 {
        let preferred = created_at.max(0) as u64;
        let mut id = self
            .store
            .as_ref()
            .and_then(|store| store.reserve_id(preferred).ok())
            .unwrap_or(preferred);
        while self.current.id == id || self.remembered.iter().any(|held| held.id == id) {
            id += 1;
        }
        id
    }

    /// Names the open calculation, or takes its name away when the field is left
    /// blank. A name is enough on its own to keep an empty calculation; losing
    /// one is enough to let it go.
    fn rename_document(&mut self, title: String, cx: &mut Context<Self>) {
        self.current.set_title(&title);
        let shown = self.current.display_title();
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.set_title(shown, cx));
        self.persist_current();
        self.refresh_search(cx);
        cx.notify();
    }

    /// Deletes a calculation, wherever it is being kept. Deleting the open one
    /// leaves nothing to show, so a fresh blank calculation takes its place and
    /// the search modal closes.
    fn delete_document(&mut self, id: u64, cx: &mut Context<Self>) {
        let held = self.remembered.iter().any(|document| document.id == id);
        self.remembered.retain(|document| document.id != id);
        let stored = self
            .store
            .as_ref()
            .is_some_and(|store| store.delete(id).is_ok());
        if !held && !stored {
            return;
        }

        if self.current.id == id {
            // Saving writes the whole record back, so the open document has to
            // be let go of entirely: both the edits waiting to be written and
            // the caret, which on its own would be enough to recreate it.
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

    /// Takes what is pending into the open calculation and keeps it wherever it
    /// now belongs. The caret travels with the text, so a calculation reopens
    /// where it was left — including when only the caret moved.
    ///
    /// Moving the caret is not an edit: the edit time only advances when the
    /// text itself changed, so merely reading a calculation doesn't reorder the
    /// search list.
    fn flush_save(&mut self, cx: &mut Context<Self>) {
        self.save_timer = None;
        let cursor = self.editor.read(cx).cursor();
        let content = self.unsaved.take();
        if content.is_none() && cursor == self.current.cursor {
            return;
        }

        if let Some(content) = content
            && content != self.current.content
        {
            self.current.content = content;
            self.current.updated_at = now_millis();
        }
        self.current.cursor = cursor;
        self.persist_current();
    }

    /// Keeps the open calculation wherever it now belongs: the store while it
    /// has content, memory while it has only a name, and nowhere at all while it
    /// has neither.
    fn persist_current(&mut self) {
        if let Some(store) = self.store.as_ref() {
            if self.current.is_empty() {
                if self.current_on_disk && store.delete(self.current.id).is_ok() {
                    self.current_on_disk = false;
                }
            } else if store.put(&self.current).is_ok() {
                self.current_on_disk = true;
            }
        }

        self.remembered
            .retain(|document| document.id != self.current.id);
        if self.current.is_empty() && self.current.is_worth_keeping() {
            self.remembered.push(self.current.clone());
        }
    }

    /// Drops edits that have not been written yet — used when their document is
    /// deleted, since saving recreates a missing record.
    fn discard_pending_save(&mut self) {
        self.save_timer = None;
        self.unsaved = None;
    }

    /// Opens or closes the help panel. Closing hands the keyboard straight back
    /// to the calculation, which is where typing carries on from — the panel's
    /// own field goes with the panel.
    fn set_help_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.help_open = open;
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.set_help_open(open, cx));
        if open {
            self.help.update(cx, HelpModal::open);
        } else {
            self.focus_editor(window, cx);
        }
        cx.notify();
    }

    /// Every calculation worth switching to — stored and remembered alike — with
    /// whatever is being typed written first so the list shows it.
    fn saved_documents(&mut self, cx: &mut Context<Self>) -> Vec<Document> {
        self.flush_save(cx);
        let mut documents = self
            .store
            .as_ref()
            .and_then(|store| store.list().ok())
            .unwrap_or_default();
        documents.extend(self.remembered.iter().cloned());
        documents.sort_by_key(|document| std::cmp::Reverse(document.updated_at));
        documents
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

    /// Hands the keyboard to whatever is in front of the calculation, or to the
    /// calculation when nothing is. The same order the window itself is focused
    /// in, so recovering focus lands where opening the window would have put it.
    fn focus_frontmost(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle(cx).focus(window);
    }

    /// Hands the keyboard back with the caret at the end, which is where typing
    /// carries on from after a calculation is created.
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
                self.set_help_open(open, window, cx);
            }
            ToolbarEvent::Rename(title) => self.rename_document(title.clone(), cx),
            ToolbarEvent::NameEditingStarted => self
                .editor
                .update(cx, |editor, cx| editor.name_editing_started(cx)),
            // Naming is an aside from typing, so the caret is left where the
            // rename interrupted it rather than moved to the end.
            ToolbarEvent::NameEditingFinished => self.focus_editor(window, cx),
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
    fn toggle_help(&mut self, _: &ToggleHelp, window: &mut Window, cx: &mut Context<Self>) {
        let open = !self.help_open;
        self.set_help_open(open, window, cx);
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
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    // The transparent native title bar leaves this strip for
                    // workspace chrome and the platform traffic lights.
                    .child(self.toolbar.clone())
                    .child(div().flex_1().child(self.editor.clone())),
            )
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
