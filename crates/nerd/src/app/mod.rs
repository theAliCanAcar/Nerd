mod find;
mod host;
mod layout;
mod shell;
mod typing;
mod widgets;
mod workspace;

pub fn serve_host() {
    host::serve();
}

use std::path::PathBuf;
use std::sync::Arc;

use nerd_terminal::{Key as TermKey, Modifiers, MouseButton, MouseEvent, SearchMatch, Session};
use eframe::egui::{
    self, Align, Button, Context, Event, Frame, ImeEvent, Key, Layout, Margin, Modal,
    MouseWheelUnit, PointerButton, RichText, Sense, Stroke, ThemePreference, Ui, Vec2,
    ViewportCommand,
};

use crate::chrome::{self, Icon};
use crate::labs;
use crate::grid::{self, CellPosition, Geometry, Selection};
use crate::input::{self, Action, MAC};
use crate::theme::{self, FONT_SIZE, MAX_FONT_SIZE, MIN_FONT_SIZE, Palette};
use crate::{ai, commands};
use find::FileSearch;
use layout::{DropSide, Screens};

const SIDEBAR_WIDTH: f32 = 210.0;
const SIDEBAR_MIN_WINDOW: f32 = 680.0;
const SEARCH_ID: &str = "terminal search";

enum ScreenRequest {
    Split { from: usize, side_by_side: bool },
    Close(usize),
}

struct Tab {
    id: usize,
    session: Option<Arc<dyn host::Shell>>,
    title: String,
    custom_title: Option<String>,
    size: (u16, u16),
    cell_pixels: (u16, u16),
    selection: Option<Selection>,
    scrolled: bool,
    error: Option<String>,
    focused: bool,
    used: bool,
}

impl Tab {
    fn display_name(&self) -> String {
        if let Some(title) = &self.custom_title {
            return title.clone();
        }
        if self.title.is_empty() {
            format!("Shell {}", self.id)
        } else {
            self.title.clone()
        }
    }

    fn focus(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            if let Some(session) = &self.session {
                let _ = session.focus(focused);
            }
        }
    }
}

#[derive(Default)]
struct Search {
    query: String,
    matches: Vec<SearchMatch>,
    index: usize,
    focus: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SettingsPage {
    #[default]
    Appearance,
    Ai,
    Shortcuts,
}

pub struct TerminalApp {
    tabs: Vec<Tab>,
    active: usize,
    next_id: usize,
    layout: Screens,
    next_split: u64,
    screen_request: Option<ScreenRequest>,
    dragging_pane: Option<usize>,
    drop_hint: Option<(usize, DropSide)>,
    tabbed: bool,
    sidebar_visible: bool,
    reveal_session: bool,
    font_size: f32,
    preference: ThemePreference,
    search: Option<Search>,
    paste: Option<String>,
    shortcuts_open: bool,
    settings_open: bool,
    settings_page: SettingsPage,
    rename: Option<String>,
    focus_rename: bool,
    cursor_style: theme::CursorStyle,
    assistant: ai::Panel,
    prompt: ai::ShellPrompt,
    examples: ai::CodeExamples,
    pending_run: Option<String>,
    pending_reason: Option<String>,
    labs: labs::Labs,
    last_undo: Option<String>,
    failure: Option<String>,
    watch_failure: bool,
    draft: String,
    phrase: Option<String>,
    phrase_job: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    file_search: FileSearch,
    provider_probe: Option<std::sync::mpsc::Receiver<bool>>,
    provider_checked: bool,
    ai_preferences: ai::Preferences,
    commands: commands::Palette,
    read_output: Option<String>,
    focus_output: bool,
    toolbar_focused: bool,
    focus_toolbar: bool,
    focus_cancel: bool,
    composition: String,
    mouse_button: Option<MouseButton>,
    mouse_cell: Option<CellPosition>,
    wheel: f32,
    selecting: bool,
    cwd: Option<PathBuf>,
    host: Option<host::HostClient>,
    #[cfg(target_os = "macos")]
    _native_menu: Option<muda::Menu>,
}

impl TerminalApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        theme::configure(ctx);
        let (preference, font_size) = theme::load_appearance(cc.storage);
        ctx.set_theme(preference);
        // Finder and other launchers may start a desktop app with `/` as its working directory.
        let cwd = std::env::current_dir()
            .ok()
            .filter(|p| p.parent().is_some())
            .or_else(|| {
                std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                    .map(PathBuf::from)
                    .filter(|p| p.is_dir())
            });
        let mut app = Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            layout: Screens::Shell(1),
            next_split: 1,
            screen_request: None,
            dragging_pane: None,
            drop_hint: None,
            tabbed: false,
            sidebar_visible: theme::load_sidebar(cc.storage),
            reveal_session: false,
            font_size,
            preference,
            search: None,
            paste: None,
            shortcuts_open: false,
            settings_open: false,
            settings_page: SettingsPage::default(),
            rename: None,
            focus_rename: false,
            cursor_style: theme::CursorStyle::load(cc.storage),
            assistant: ai::Panel::default(),
            prompt: ai::ShellPrompt::default(),
            examples: ai::CodeExamples::default(),
            pending_run: None,
            pending_reason: None,
            labs: labs::Labs::default(),
            last_undo: None,
            failure: None,
            watch_failure: false,
            draft: String::new(),
            phrase: None,
            phrase_job: None,
            file_search: FileSearch::default(),
            provider_probe: None,
            provider_checked: false,
            ai_preferences: ai::Preferences::load(cc.storage),
            commands: commands::Palette::default(),
            read_output: None,
            focus_output: false,
            toolbar_focused: false,
            focus_toolbar: false,
            focus_cancel: false,
            composition: String::new(),
            mouse_button: None,
            mouse_cell: None,
            wheel: 0.0,
            selecting: false,
            cwd: cwd.clone(),
            host: host::connect(ctx),
            #[cfg(target_os = "macos")]
            _native_menu: crate::macos_menu::install(ctx)
                .map_err(|error| eprintln!("Could not initialize the macOS menu bar: {error}"))
                .ok(),
        };
        if let Some(client) = app.host.clone() {
            for shell in client.running() {
                let id = app.next_id;
                app.next_id += 1;
                app.tabs.push(attached_tab(id, shell));
            }
        }
        if app.tabs.is_empty() {
            app.new_tab(ctx);
        }
        let ids: Vec<usize> = app.tabs.iter().map(|tab| tab.id).collect();
        let mut split_id = 1;
        app.layout = Screens::balanced(&ids, &mut split_id);
        app.next_split = split_id;
        app
    }

    fn poll_phrase(&mut self, ctx: &Context) {
        if self.phrase_job.is_none() {
            if let Some(line) = self.phrase.take() {
                if let Some(route) = self.ai_preferences.route() {
                    let prompt = format!(
                        "The user typed this in their own language. It may be any language and it is not a shell command: \"{line}\". Understand the meaning and reply with one fenced block that contains only the safest matching shell command. Do not run it."
                    );
                    let (tx, rx) = std::sync::mpsc::channel();
                    let ctx = ctx.clone();
                    let _ = std::thread::Builder::new()
                        .name("nerd-phrase".into())
                        .spawn(move || {
                            let _ = tx.send(ai::request(&route, &prompt));
                            ctx.request_repaint();
                        });
                    self.phrase_job = Some(rx);
                }
            }
        }
        let ready = self.phrase_job.as_ref().and_then(|job| match job.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(Err("The model request stopped.".into()))
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        });
        if let Some(result) = ready {
            self.phrase_job = None;
            if let Ok(answer) = result {
                if let Some(command) = ai::suggested_command(&answer) {
                    self.insert_command(command);
                }
            }
        }
    }

    fn finish_run(&mut self, command: String) {
        self.last_undo = Some(ai::undo_note(&command));
        self.watch_failure = true;
        self.failure = None;
        self.run_command(command);
    }

    fn run_command(&mut self, command: String) {
        let tab = &mut self.tabs[self.active];
        tab.used = true;
        if let Some(session) = &tab.session {
            let mut bytes = command.into_bytes();
            bytes.push(b'\r');
            if let Err(error) = session.write(&bytes) {
                tab.error = Some(format!("Could not run the command: {error:#}"));
            }
        }
    }

    fn insert_command(&mut self, command: String) {
        let tab = &mut self.tabs[self.active];
        if let Some(session) = &tab.session {
            if let Err(error) = session.paste(&command) {
                tab.error = Some(format!("Could not insert the command: {error:#}"));
            }
        }
    }

    fn new_tab(&mut self, ctx: &Context) {
        if let Some(index) = self.tabs.iter().position(|tab| !tab.used) {
            self.activate(index);
            self.reveal_session = true;
            return;
        }
        let beside = self.tabs.get(self.active).map(|tab| tab.id);
        self.split_from(ctx, beside, true);
    }

    fn split_from(&mut self, ctx: &Context, beside: Option<usize>, side_by_side: bool) {
        let size = self.tabs.get(self.active).map_or((100, 28), |tab| tab.size);
        let tab = self.make_tab(ctx, self.next_id, size);
        self.next_id += 1;
        let id = tab.id;
        self.tabs.push(tab);
        match beside.filter(|current| self.layout.contains(*current)) {
            Some(current) => {
                let split_id = self.next_split;
                self.next_split += 1;
                self.layout.split_around(current, id, side_by_side, split_id);
            }
            None => self.layout = Screens::Shell(id),
        }
        self.activate(self.tabs.len() - 1);
    }
}

fn attached_tab(id: usize, shell: Arc<dyn host::Shell>) -> Tab {
    Tab {
        id,
        session: Some(shell),
        scrolled: false,
        title: String::new(),
        custom_title: None,
        size: (80, 24),
        cell_pixels: (0, 0),
        selection: None,
        error: None,
        focused: false,
        used: true,
    }
}

impl TerminalApp {
    fn make_tab(&self, ctx: &Context, id: usize, size: (u16, u16)) -> Tab {
        let (session, error) = self.start_shell(ctx, size);
        Tab {
            id,
            session,
            scrolled: false,
            title: String::new(),
            custom_title: None,
            size,
            cell_pixels: (0, 0),
            selection: None,
            error,
            focused: false,
            used: false,
        }
    }

    fn start_shell(
        &self,
        ctx: &Context,
        size: (u16, u16),
    ) -> (Option<Arc<dyn host::Shell>>, Option<String>) {
        if let Some(client) = &self.host {
            match client.open(size.0, size.1, self.cwd.as_deref()) {
                Ok(shell) => return (Some(shell), None),
                Err(error) => {
                    eprintln!("Could not reach the session host, opening a shell in this window: {error:#}")
                }
            }
        }
        let ui_ctx = ctx.clone();
        match Session::spawn(size.0, size.1, self.cwd.as_deref(), move || {
            ui_ctx.request_repaint();
        }) {
            Ok(session) => (Some(Arc::new(session)), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        }
    }

    fn close_tab(&mut self, ctx: &Context, index: usize) {
        let exited = self.tabs[index]
            .session
            .as_ref()
            .is_none_or(|session| session.exited());
        if self.tabs.len() == 1 && !exited {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        if let Some(session) = &self.tabs[index].session {
            session.end();
        }
        if self.tabs.len() == 1 {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        let closing_active = self.active == index;
        let id = self.tabs[index].id;
        self.layout.remove_shell(id);
        self.tabs.remove(index);
        if closing_active {
            let leaf = self.layout.first_leaf();
            self.active = leaf
                .and_then(|id| self.tabs.iter().position(|tab| tab.id == id))
                .unwrap_or(0);
            self.reset_interaction();
        } else if self.active > index {
            self.active -= 1;
        }
    }

    fn activate(&mut self, index: usize) {
        if index != self.active {
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.focus(false);
            }
            self.active = index;
            self.reset_interaction();
        }
    }

    fn reset_interaction(&mut self) {
        self.reveal_session = true;
        self.search = None;
        self.composition.clear();
        self.mouse_button = None;
        self.mouse_cell = None;
        self.wheel = 0.0;
        self.selecting = false;
        self.toolbar_focused = false;
    }

    fn action(&mut self, ctx: &Context, action: Action) {
        match action {
            Action::NewTab => self.new_tab(ctx),
            Action::CloseTab => self.close_tab(ctx, self.active),
            Action::SplitRight => {
                let beside = self.tabs.get(self.active).map(|tab| tab.id);
                self.split_from(ctx, beside, true);
            }
            Action::SplitDown => {
                let beside = self.tabs.get(self.active).map(|tab| tab.id);
                self.split_from(ctx, beside, false);
            }
            Action::Search => {
                self.toolbar_focused = false;
                self.search.get_or_insert_with(Search::default).focus = true;
            }
            Action::ToggleSidebar if ctx.content_rect().width() >= SIDEBAR_MIN_WINDOW => {
                self.sidebar_visible = !self.sidebar_visible;
                self.reveal_session = self.sidebar_visible;
            }
            Action::ToggleSidebar => {}
            Action::Settings => self.settings_open = true,
            Action::AskAi => self.assistant.summon(),
            Action::FindPath => {
                self.file_search.open = true;
                self.file_search.focus = true;
            }
            Action::CommandPalette => self.commands.open(),
            Action::RenameSession => {
                self.rename = Some(
                    self.tabs[self.active]
                        .custom_title
                        .clone()
                        .unwrap_or_default(),
                );
                self.focus_rename = true;
            }
            Action::Shortcuts => self.shortcuts_open = true,
            Action::ReadOutput => {
                if let Some(session) = &self.tabs[self.active].session {
                    self.read_output = Some(grid::plain_text(&session.snapshot()));
                    self.focus_output = true;
                }
            }
            Action::FocusToolbar => {
                self.toolbar_focused = !self.toolbar_focused;
                self.focus_toolbar = self.toolbar_focused;
                self.search = None;
            }
            Action::Copy => {
                let tab = &self.tabs[self.active];
                if let (Some(session), Some(selection)) = (&tab.session, tab.selection) {
                    let text = session.copy(selection.anchor, selection.end);
                    if !text.is_empty() {
                        ctx.copy_text(text);
                    }
                }
            }
            Action::Paste => ctx.send_viewport_cmd(ViewportCommand::RequestPaste),
            Action::PreviousTab => {
                self.activate((self.active + self.tabs.len() - 1) % self.tabs.len())
            }
            Action::NextTab => self.activate((self.active + 1) % self.tabs.len()),
            Action::ZoomIn => self.font_size = (self.font_size + 1.0).min(MAX_FONT_SIZE),
            Action::ZoomOut => self.font_size = (self.font_size - 1.0).max(MIN_FONT_SIZE),
            Action::ResetZoom => self.font_size = FONT_SIZE,
        }
    }
}

impl eframe::App for TerminalApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        theme::save_appearance(storage, self.preference, self.font_size);
        theme::save_sidebar(storage, self.sidebar_visible);
        self.ai_preferences.save(storage);
        self.cursor_style.save(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        if !self.provider_checked && self.ai_preferences.model().is_none() && !self.ai_preferences.wizard {
            self.provider_checked = true;
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx = ctx.clone();
            let _ = std::thread::Builder::new().name("nerd-ollama-probe".into()).spawn(move || {
                let _ = tx.send(ai::ollama_reachable());
                ctx.request_repaint();
            });
            self.provider_probe = Some(rx);
        }
        if let Some(probe) = &self.provider_probe {
            if let Ok(running) = probe.try_recv() {
                self.provider_probe = None;
                if !running && self.ai_preferences.model().is_none() {
                    self.ai_preferences.open_wizard();
                }
            }
        }
        let modal_open = self.paste.is_some()
            || self.shortcuts_open
            || self.file_search.open
            || self.settings_open
            || self.read_output.is_some()
            || self.assistant.open
            || self.prompt.open
            || self.examples.open
            || self.pending_run.is_some()
            || self.labs.open
            || self.ai_preferences.wizard
            || self.commands.open
            || self.rename.is_some();
        #[cfg(target_os = "macos")]
        crate::macos_menu::dispatch(ctx, |action| {
            if !modal_open {
                self.action(ctx, action);
            }
        });
        let mut events = ctx.input(|i| i.events.clone());
        if !modal_open {
            if self.toolbar_focused && ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.toolbar_focused = false;
                events.clear();
            }
            events.retain(|event| match event {
                Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } => {
                    let Some(action) = input::app_shortcut(*key, *modifiers) else {
                        return true;
                    };
                    if !repeat || matches!(action, Action::ZoomIn | Action::ZoomOut) {
                        self.action(ctx, action);
                    }
                    false
                }
                Event::Text(text) if text == " " || text == "\u{00a0}" => {
                    let modifiers = ctx.input(|input| input.modifiers);
                    if modifiers.shift && !modifiers.command && !modifiers.ctrl && !modifiers.alt {
                        self.action(ctx, Action::AskAi);
                        false
                    } else {
                        true
                    }
                }
                _ => true,
            });
        }
        let palette = theme::current(ctx);
        let use_sidebar = self.sidebar_visible && ctx.content_rect().width() >= SIDEBAR_MIN_WINDOW;
        if use_sidebar {
            self.sidebar(ctx, palette);
            self.workspace_header(ctx, palette);
        } else {
            self.toolbar(ctx, palette);
        }
        let searching = self.search.is_some();
        self.search_bar(ctx, palette);
        self.shortcut_bar(ctx, palette);
        if searching || modal_open {
            events.clear();
        }
        if let Some(overlay) = self.status_overlay(ctx, palette, use_sidebar) {
            events.retain(|event| !matches!(event, Event::PointerButton { pos, pressed: true, .. } if overlay.contains(*pos)));
        }
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(if use_sidebar {
                        palette.workspace
                    } else {
                        palette.terminal
                    })
                    .inner_margin(if use_sidebar { 13 } else { 0 }),
            )
            .show(ctx, |ui| {
                let pane = ui.max_rect();
                if use_sidebar {
                    ui.painter().rect(
                        pane,
                        10,
                        palette.terminal,
                        Stroke::new(1.0, palette.border),
                        egui::StrokeKind::Inside,
                    );
                }
                self.terminal(ui, palette, &events);
            });
        self.poll_phrase(ctx);
        self.modal(ctx);
    }
}
