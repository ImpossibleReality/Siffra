use super::toolbar::{Toolbar, ToolbarEvent};
use crate::documents::{Document, DocumentStore, database_path, now_millis};
use crate::editor::layout::Fonts;
use crate::editor::{ContentChanged, Editor};
use crate::help::{CloseHelp, HelpModal};
use crate::search::{SearchEvent, SearchModal};
use crate::updater;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Subscription, Task, Window, actions, div, px,
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

const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

pub struct Workspace {
    store: Option<DocumentStore>,
    current: Document,
    current_on_disk: bool,
    remembered: Vec<Document>,
    editor: Entity<Editor>,
    toolbar: Entity<Toolbar>,
    help: Entity<HelpModal>,
    help_open: bool,
    search: Entity<SearchModal>,
    search_open: bool,
    unsaved: Option<String>,
    save_timer: Option<Task<()>>,
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
            cx.observe_window_activation(window, |workspace, window, cx| {
                if !window.is_window_active() {
                    workspace.flush_save(cx);
                }
            }),
            cx.on_focus_lost(window, |workspace, window, cx| {
                workspace.focus_frontmost(window, cx);
            }),
        ];
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

        workspace.show_blank(cx);
        workspace
    }

    pub fn editor_focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle(cx)
    }

    pub fn flush_pending_save(&mut self, cx: &mut Context<Self>) {
        self.flush_save(cx);
    }

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

    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_save(cx);
        self.show_blank(cx);
        self.focus_editor_end(window, cx);
    }

    fn show_blank(&mut self, cx: &mut Context<Self>) {
        let created_at = now_millis();
        let document = Document::new(self.unused_id(created_at), created_at);
        self.discard_pending_save();
        self.open_in_editor(document, false, cx);
    }

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

    fn rename_document(&mut self, title: String, cx: &mut Context<Self>) {
        self.current.set_title(&title);
        let shown = self.current.display_title();
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.set_title(shown, cx));
        self.persist_current();
        self.refresh_search(cx);
        cx.notify();
    }

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

    fn discard_pending_save(&mut self) {
        self.save_timer = None;
        self.unsaved = None;
    }

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

    fn refresh_search(&mut self, cx: &mut Context<Self>) {
        if !self.search_open {
            return;
        }
        let documents = self.saved_documents(cx);
        self.search
            .update(cx, |search, cx| search.set_documents(documents, cx));
    }

    fn focus_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |editor, _| editor.focus(window));
    }

    fn focus_frontmost(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle(cx).focus(window);
    }

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

    fn name_document(&mut self, _: &NameDocument, window: &mut Window, cx: &mut Context<Self>) {
        self.close_search(cx);
        self.toolbar
            .update(cx, |toolbar, cx| toolbar.start_editing(window, cx));
    }

    fn create_document(&mut self, _: &NewDocument, window: &mut Window, cx: &mut Context<Self>) {
        self.close_search(cx);
        self.new_document(window, cx);
    }

    fn toggle_help(&mut self, _: &ToggleHelp, window: &mut Window, cx: &mut Context<Self>) {
        let open = !self.help_open;
        self.set_help_open(open, window, cx);
    }

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
                    .child(self.toolbar.clone())
                    .child(div().flex_1().min_h(px(0.0)).child(self.editor.clone())),
            )
            .children(self.help_open.then(|| self.help.clone()))
            .children(self.search_open.then(|| self.search.clone()))
    }
}

impl Focusable for Workspace {
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
