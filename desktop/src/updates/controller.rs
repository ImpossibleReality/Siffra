use super::{
    AUTO_UPDATE_REFRESH_INTERVAL, CheckResult, CheckState, UpdateModal, UpdateModalEvent,
    UpdateState, VersionButton, VersionButtonEvent, check,
};
use gpui::{AppContext, Context, Entity, EventEmitter, SharedString, Subscription, Task, Window};
use jiff::Timestamp;

/// A notification for the workspace that owns the update UI.
pub enum UpdatesEvent {
    StateChanged,
    Toast(SharedString),
    ModalClosed,
}

/// Coordinates update checks, installation, and the views that present them.
pub struct Updates {
    state: UpdateState,
    version: Entity<VersionButton>,
    modal: Entity<UpdateModal>,
    modal_open: bool,
    update_task: Option<Task<()>>,
    launched_version: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

/// Who asked for an update check, which decides where its outcome is reported.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckOrigin {
    Launch,
    VersionMenu,
    AppMenu,
}

impl EventEmitter<UpdatesEvent> for Updates {}

impl Updates {
    pub fn new(mono_family: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state = UpdateState::load();
        let launched_version = state.record_launch();
        let version = cx.new(|_| VersionButton::new(state.status()));
        let modal = cx.new(|cx| UpdateModal::new(mono_family, cx));
        let subscriptions = vec![
            cx.subscribe_in(&version, window, Self::on_version_event),
            cx.subscribe_in(&modal, window, Self::on_modal_event),
        ];

        Self {
            state,
            version,
            modal,
            modal_open: false,
            update_task: None,
            launched_version,
            _subscriptions: subscriptions,
        }
    }

    pub fn version(&self) -> Entity<VersionButton> {
        self.version.clone()
    }

    pub fn open_modal(&self) -> Option<Entity<UpdateModal>> {
        self.modal_open.then(|| self.modal.clone())
    }

    pub fn modal_is_open(&self) -> bool {
        self.modal_open
    }

    pub fn update_announced(&self) -> bool {
        let status = self.state.status();
        status.announced && !status.auto_update
    }

    pub fn launched_version(&self) -> Option<SharedString> {
        self.launched_version.clone()
    }

    pub fn check_on_launch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.auto_update()
            && self.state.last_check().is_none_or(|ts| {
                (Timestamp::now() - ts).get_hours() > 24 * AUTO_UPDATE_REFRESH_INTERVAL
            })
        {
            self.start_check(CheckOrigin::Launch, window, cx);
        }
    }

    pub fn check_from_app_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.release().is_some() {
            self.show_update(window, cx);
        } else {
            self.start_check(CheckOrigin::AppMenu, window, cx);
        }
    }

    fn sync_views(&mut self, cx: &mut Context<Self>) {
        self.version.update(cx, |version, cx| {
            version.set_status(self.state.status(), cx)
        });
        cx.emit(UpdatesEvent::StateChanged);
    }

    fn start_check(&mut self, origin: CheckOrigin, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_task.is_some() {
            return;
        }
        self.set_check_state(CheckState::Checking, cx);
        if origin == CheckOrigin::AppMenu {
            cx.emit(UpdatesEvent::Toast(SharedString::new_static(
                "Checking for updates…",
            )));
        }

        let work = cx.background_spawn(async { check() });
        self.update_task = Some(cx.spawn_in(window, async move |updates, cx| {
            let result = work.await;
            updates
                .update_in(cx, |updates, window, cx| {
                    updates.update_task = None;
                    updates.finish_check(origin, result, window, cx);
                })
                .ok();
        }));
    }

    fn finish_check(
        &mut self,
        origin: CheckOrigin,
        result: CheckResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let announce = origin == CheckOrigin::AppMenu;
        self.set_check_state(
            if result.is_err() {
                CheckState::Failed
            } else {
                CheckState::Idle
            },
            cx,
        );

        match result {
            Ok(Some(release)) => {
                self.state.set_last_check(Timestamp::now());
                self.state.set_release(release);
                self.sync_views(cx);
                if announce {
                    self.show_update(window, cx);
                } else if self.state.auto_update() {
                    self.install_update(window, cx);
                }
            }
            Ok(None) if announce => {
                self.state.set_last_check(Timestamp::now());
                cx.emit(UpdatesEvent::Toast(SharedString::new_static(
                    "Siffra is up to date.",
                )));
            }
            Ok(None) => self.state.set_last_check(Timestamp::now()),
            Err(error) if announce => cx.emit(UpdatesEvent::Toast(SharedString::from(format!(
                "Could not check for updates: {error}",
            )))),
            Err(_) => {}
        }
    }

    fn set_check_state(&mut self, check: CheckState, cx: &mut Context<Self>) {
        self.version
            .update(cx, |version, cx| version.set_check_state(check, cx));
    }

    fn show_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(release) = self.state.release() else {
            return;
        };
        self.modal_open = true;
        self.modal.update(cx, |modal, cx| {
            modal.open(
                self.state.status().current_version,
                release.version.clone(),
                &release.notes,
                window,
                cx,
            )
        });
        cx.emit(UpdatesEvent::StateChanged);
    }

    fn close_update(&mut self, cx: &mut Context<Self>) {
        if !self.modal_open {
            return;
        }
        self.modal_open = false;
        cx.emit(UpdatesEvent::StateChanged);
        cx.emit(UpdatesEvent::ModalClosed);
    }

    fn install_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(release) = self.state.release() else {
            return;
        };
        if self.update_task.is_some() {
            return;
        }
        self.modal
            .update(cx, |modal, cx| modal.set_installing(true, cx));
        let work = cx.background_spawn(async move { release.install() });
        self.update_task = Some(cx.spawn_in(window, async move |updates, cx| {
            let result = work.await;
            updates
                .update(cx, |updates, cx| {
                    updates.update_task = None;
                    updates
                        .modal
                        .update(cx, |modal, cx| modal.set_installing(false, cx));
                    if let Err(error) = result {
                        cx.emit(UpdatesEvent::Toast(SharedString::from(format!(
                            "Could not install the update: {error}",
                        ))));
                    }
                })
                .ok();
        }));
    }

    fn on_version_event(
        &mut self,
        _: &Entity<VersionButton>,
        event: &VersionButtonEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            VersionButtonEvent::CheckForUpdates => {
                self.start_check(CheckOrigin::VersionMenu, window, cx)
            }
            VersionButtonEvent::ShowUpdate => self.show_update(window, cx),
            VersionButtonEvent::SetAutoUpdate(on) => {
                self.state.set_auto_update(*on);
                self.sync_views(cx);
            }
        }
    }

    fn on_modal_event(
        &mut self,
        _: &Entity<UpdateModal>,
        event: &UpdateModalEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            UpdateModalEvent::Dismiss { never_again } => {
                if *never_again {
                    self.state.ignore_release();
                    self.sync_views(cx);
                }
                self.close_update(cx);
            }
            UpdateModalEvent::Install => self.install_update(window, cx),
        }
    }
}
