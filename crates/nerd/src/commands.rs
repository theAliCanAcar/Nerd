use eframe::egui::{self, Context, Key, Modal, TextEdit};

use crate::input::{Action, MAC};

const COMMANDS: &[(&str, &str, &str, Action)] = &[
    ("New Session", "⌘T", "Ctrl+Shift+T", Action::NewTab),
    ("Split Right", "⌘D", "Ctrl+Shift+D", Action::SplitRight),
    ("Split Down", "⌘⇧D", "Ctrl+Alt+D", Action::SplitDown),
    ("Rename Session", "", "", Action::RenameSession),
    ("Find in Scrollback", "⌘F", "Ctrl+Shift+F", Action::Search),
    ("Ask AI", "Shift+Space", "Shift+Space", Action::AskAi),
    ("Open Settings", "⌘,", "", Action::Settings),
    (
        "Toggle Sidebar",
        "⌥⌘S",
        "Ctrl+Shift+B",
        Action::ToggleSidebar,
    ),
    ("Read Visible Output", "", "", Action::ReadOutput),
    ("Increase Text Size", "⌘+", "Ctrl+Plus", Action::ZoomIn),
    ("Decrease Text Size", "⌘−", "Ctrl+Minus", Action::ZoomOut),
    ("Reset Text Size", "⌘0", "Ctrl+0", Action::ResetZoom),
    ("Keyboard Shortcuts", "", "", Action::Shortcuts),
];

#[derive(Default)]
pub struct Palette {
    pub open: bool,
    focus: bool,
    query: String,
    selected: usize,
}

impl Palette {
    pub fn open(&mut self) {
        self.open = true;
        self.focus = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn show(&mut self, ctx: &Context) -> Option<Action> {
        if !self.open {
            return None;
        }
        let response = Modal::new(egui::Id::new("command palette")).show(ctx, |ui| {
            let width = (ctx.content_rect().width() - 48.0).clamp(320.0, 460.0);
            ui.set_width(width);
            ui.heading("Commands");
            let query = ui.add(
                TextEdit::singleline(&mut self.query)
                    .desired_width(width)
                    .hint_text("Search actions…"),
            );
            if std::mem::take(&mut self.focus) {
                query.request_focus();
            }
            if query.changed() {
                self.selected = 0;
            }
            let lower = self.query.trim().to_lowercase();
            let matching = COMMANDS
                .iter()
                .filter(|(title, _, _, _)| title.to_lowercase().contains(&lower))
                .collect::<Vec<_>>();
            if matching.is_empty() {
                ui.label("No matching commands");
                return None;
            }
            let movement = ui.input(|i| {
                i32::from(i.key_pressed(Key::ArrowDown)) - i32::from(i.key_pressed(Key::ArrowUp))
            });
            self.selected =
                (self.selected as i32 + movement).rem_euclid(matching.len() as i32) as usize;
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                return Some(matching[self.selected].3);
            }
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 115.0).max(85.0))
                .show(ui, |ui| {
                    for (index, (title, mac, other, action)) in matching.into_iter().enumerate() {
                        let button = egui::Button::selectable(index == self.selected, *title)
                            .shortcut_text(if MAC { *mac } else { *other });
                        let item = ui.add_sized([ui.available_width(), 31.0], button);
                        if index == self.selected && movement != 0 {
                            item.scroll_to_me(None);
                        }
                        if item.clicked() {
                            return Some(*action);
                        }
                    }
                    None
                })
                .inner
        });
        if response.inner.is_some() || response.should_close() {
            self.open = false;
        }
        response.inner
    }
}
