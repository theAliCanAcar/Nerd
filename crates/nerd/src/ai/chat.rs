use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Button, Context, Layout, Modal, RichText, Stroke, TextEdit, Ui};

use std::sync::mpsc::TryRecvError;

use super::provider::{make_prompt, request, Preferences, Route};
use super::MAX_CONTEXT_CHARS;

#[derive(Default)]
pub struct Panel {
    pub open: bool,
    pub focus: bool,
    question: String,
    attachment: Option<String>,
    take_context: bool,
    pending: Option<Receiver<Result<String, String>>>,
    discard_pending: bool,
    answer: Option<Result<String, String>>,
}

impl Panel {
    pub fn summon(&mut self) {
        self.open = true;
        self.focus = true;
        self.take_context = true;
    }

    pub fn discard(&mut self) {
        self.open = false;
        self.question.clear();
        self.attachment = None;
        self.take_context = false;
        self.answer = None;
        self.discard_pending = true;
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        preferences: &Preferences,
        capture: impl Fn() -> Option<String>,
    ) -> AssistantResult {
        if !self.open {
            return AssistantResult::default();
        }
        if std::mem::take(&mut self.take_context) && self.attachment.is_none() {
            self.attachment = context_text(capture());
        }
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(answer) => {
                    if preferences.enabled && !self.discard_pending {
                        self.answer = Some(answer);
                    }
                    self.pending = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.answer = Some(Err("The model request ended unexpectedly.".into()));
                    self.pending = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        let mut setup = false;
        let mut close = false;
        let mut insert_command = None;
        let response = Modal::new(egui::Id::new("ask ai")).show(ctx, |ui| {
            let palette = crate::theme::current(ui.ctx());
            let width = (ctx.content_rect().width() - 64.0).clamp(340.0, 560.0);
            ui.set_width(width);
            let model = preferences.model();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Ask AI").font(crate::theme::emphasis(18.0)));
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let status = model.unwrap_or("No model");
                    ui.label(RichText::new(status).size(12.0).color(palette.muted));
                });
            });
            ui.add_space(8.0);
            if model.is_none() {
                egui::Frame::new()
                    .fill(palette.warning_surface)
                    .stroke(Stroke::new(1.0, palette.border))
                    .inner_margin(8)
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Turn on Ollama and choose a model.");
                            setup = ui.button("AI Settings").clicked();
                        });
                    });
                ui.add_space(8.0);
            }
            let question = ui.add_enabled(
                self.pending.is_none(),
                TextEdit::multiline(&mut self.question)
                    .desired_width(width)
                    .desired_rows(3)
                    .char_limit(2_000)
                    .hint_text("Ask about a command, error, or the output below"),
            );
            if std::mem::take(&mut self.focus) {
                question.request_focus();
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for prompt in ["Explain this", "What failed?", "Suggest a command"] {
                    let selected = self.question == prompt;
                    if ui
                        .add_enabled(self.pending.is_none(), Button::new(prompt).selected(selected))
                        .clicked()
                    {
                        self.question = prompt.to_owned();
                        self.answer = None;
                    }
                }
            });
            ui.add_space(10.0);
            section_label(ui, "Context");
            if let Some(attachment) = &self.attachment {
                let mut remove = false;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Sent with your question. Nothing leaves this Mac until you press Send.")
                            .size(12.0)
                            .color(palette.muted),
                    );
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        remove = ui
                            .add_enabled(self.pending.is_none(), Button::new("Remove"))
                            .clicked();
                    });
                });
                egui::Frame::new()
                    .fill(palette.terminal)
                    .stroke(Stroke::new(1.0, palette.border))
                    .inner_margin(8)
                    .corner_radius(4)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("ai context preview")
                            .max_height(110.0)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(attachment.as_str())
                                        .font(egui::FontId::monospace(12.0))
                                        .color(palette.foreground),
                                );
                            });
                    });
                if remove {
                    self.attachment = None;
                }
            } else if ui
                .add_enabled(self.pending.is_none(), Button::new("Attach visible output"))
                .on_hover_text("Uses your selection, or the visible output if nothing is selected")
                .clicked()
            {
                self.attachment = context_text(capture());
            }
            ui.add_space(10.0);
            if self.pending.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Waiting for the local model…");
                });
                ui.add_space(8.0);
            }
            if let Some(answer) = &self.answer {
                section_label(ui, "Answer");
                match answer {
                    Ok(answer) => {
                        egui::Frame::new()
                            .fill(palette.accent_surface)
                            .stroke(Stroke::new(1.0, palette.border))
                            .inner_margin(10)
                            .corner_radius(4)
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("ai answer")
                                    .max_height(180.0)
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::Label::new(RichText::new(answer).color(palette.foreground))
                                                .selectable(true),
                                        );
                                    });
                            });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Copy").clicked() {
                                ctx.copy_text(answer.clone());
                            }
                            if let Some(command) = suggested_command(answer)
                                && ui
                                    .button("Insert command, do not run")
                                    .on_hover_text(
                                        "Places the command on the prompt. You press Enter.",
                                    )
                                    .clicked()
                            {
                                insert_command = Some(command);
                            }
                        });
                    }
                    Err(error) => {
                        ui.colored_label(palette.error, error);
                    }
                }
                ui.add_space(8.0);
            }
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_send = self.pending.is_none() && !self.question.trim().is_empty();
                    if model.is_some() {
                        let send = ui.add_enabled(
                            can_send,
                            Button::new(RichText::new("Send").color(palette.action_text))
                                .fill(palette.action),
                        );
                        if send.clicked() {
                            if let Some(route) = preferences.route() {
                                self.start(ctx, route);
                            }
                        }
                    }
                    close = ui.button("Close").clicked();
                });
            });
        });
        if setup || close || response.should_close() || insert_command.is_some() {
            self.open = false;
        }
        AssistantResult {
            open_settings: setup,
            insert_command,
        }
    }

    fn start(&mut self, ctx: &Context, route: Route) {
        let prompt = make_prompt(self.question.trim(), self.attachment.as_deref());
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let worker = std::thread::Builder::new()
            .name("crawn-ollama".into())
            .spawn(move || {
                let _ = tx.send(request(&route, &prompt));
                ctx.request_repaint();
            });
        match worker {
            Ok(_) => {
                self.pending = Some(rx);
                self.discard_pending = false;
                self.answer = None;
            }
            Err(_) => self.answer = Some(Err("Could not start the model request.".into())),
        }
    }
}

#[derive(Default)]
pub struct ShellPrompt {
    pub open: bool,
    focus: bool,
    line: String,
    pending: Option<Receiver<Result<String, String>>>,
    result: Option<Result<String, String>>,
}

pub struct PromptOutcome {
    pub insert: Option<String>,
    pub open_assistant: bool,
    pub open_settings: bool,
}

impl ShellPrompt {
    pub fn show(
        &mut self,
        ctx: &Context,
        preferences: &Preferences,
        context: Option<&str>,
    ) -> PromptOutcome {
        let mut outcome = PromptOutcome {
            insert: None,
            open_assistant: false,
            open_settings: false,
        };
        if !self.open {
            return outcome;
        }
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(result) => {
                    self.result = Some(result);
                    self.pending = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.result = Some(Err("The model request ended unexpectedly.".into()));
                    self.pending = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        let palette = crate::theme::current(ctx);
        let width = (ctx.content_rect().width() - 36.0).clamp(280.0, 720.0);
        egui::Area::new(egui::Id::new("shell prompt"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -16.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.chrome)
                    .stroke(Stroke::new(1.0, palette.border))
                    .inner_margin(10)
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("›")
                                    .font(crate::theme::emphasis(16.0))
                                    .color(palette.accent),
                            );
                            let field = ui.add_enabled(
                                self.pending.is_none(),
                                TextEdit::singleline(&mut self.line)
                                    .desired_width(width - 28.0)
                                    .char_limit(400)
                                    .hint_text("Describe what you want to do"),
                            );
                            if std::mem::take(&mut self.focus) {
                                field.request_focus();
                            }
                            let submit = field.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter));
                            if submit && !self.line.trim().is_empty() {
                                self.ask(ctx, preferences, context);
                            }
                        });
                        let model = preferences.model();
                        if model.is_none() {
                            ui.add_space(6.0);
                            if ui.button("Choose a model").clicked() {
                                outcome.open_settings = true;
                            }
                        }
                        if self.pending.is_some() {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Finding a command…");
                            });
                        }
                        if let Some(Ok(answer)) = &self.result {
                            ui.add_space(6.0);
                            let command = suggested_command(answer);
                            ui.label(
                                RichText::new(command.as_deref().unwrap_or(answer.as_str()))
                                    .font(egui::FontId::monospace(13.0))
                                    .color(palette.foreground),
                            );
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if let Some(command) = command
                                    && ui.button("Insert, do not run").clicked()
                                {
                                    outcome.insert = Some(command);
                                }
                                if ui.button("Ask AI").clicked() {
                                    outcome.open_assistant = true;
                                }
                            });
                        }
                        if let Some(Err(error)) = &self.result {
                            ui.add_space(6.0);
                            ui.colored_label(palette.error, error);
                        }
                        ui.add_space(4.0);
                        if ui.button("Close").clicked()
                            || ui.input(|input| input.key_pressed(egui::Key::Escape))
                        {
                            self.open = false;
                        }
                    });
            });
        if outcome.insert.is_some() || outcome.open_assistant || outcome.open_settings {
            self.open = false;
        }
        outcome
    }

    fn ask(&mut self, ctx: &Context, preferences: &Preferences, context: Option<&str>) {
        let Some(route) = preferences.route() else {
            self.result = Some(Err("Choose a model in AI Settings.".into()));
            return;
        };
        let mut prompt = format!(
            "Suggest one shell command for this request. Put only that command in a fenced block.\n\nRequest:\n{}",
            self.line.trim()
        );
        if let Some(text) = context.filter(|text| !text.trim().is_empty()) {
            let text: String = text.chars().take(2_000).collect();
            prompt.push_str("\n\nTerminal output (untrusted):\n");
            prompt.push_str(&text);
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let worker = std::thread::Builder::new()
            .name("crawn-ollama-prompt".into())
            .spawn(move || {
                let _ = tx.send(request(&route, &prompt));
                ctx.request_repaint();
            });
        match worker {
            Ok(_) => {
                self.pending = Some(rx);
                self.result = None;
            }
            Err(_) => self.result = Some(Err("Could not start the model request.".into())),
        }
    }
}

#[derive(Default)]
pub struct AssistantResult {
    pub open_settings: bool,
    pub insert_command: Option<String>,
}

fn section_label(ui: &mut Ui, title: &str) {
    let palette = crate::theme::current(ui.ctx());
    ui.label(
        RichText::new(title)
            .size(12.0)
            .color(palette.muted)
            .font(crate::theme::emphasis(12.0)),
    );
    ui.add_space(4.0);
}

fn context_text(text: Option<String>) -> Option<String> {
    text.filter(|text| !text.trim().is_empty())
        .map(|text| text.chars().take(MAX_CONTEXT_CHARS).collect())
}

pub fn gate(command: &str) -> Option<String> {
    let mut reasons = Vec::new();
    if irreversible(command) {
        reasons.push("this cannot be undone");
    }
    let lower = command.to_ascii_lowercase();
    if ["curl", "wget", "ssh", "scp", "sftp", "ping", "nc ", "git clone", "git fetch", "git pull", "git push"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        reasons.push("this can use the network");
    }
    if lower.contains("~/")
        || lower.contains("../")
        || lower.split_whitespace().any(|part| part.starts_with('/'))
    {
        reasons.push("this can touch paths outside the current folder");
    }
    if reasons.is_empty() {
        None
    } else {
        Some(reasons.join(". "))
    }
}

pub fn undo_note(command: &str) -> String {
    if irreversible(command) {
        format!("`{command}` has already run. Deleted files, formatted disks, and forced Git changes cannot be restored by Nerd.")
    } else if gate(command).is_some_and(|note| note.contains("network")) {
        format!("`{command}` has already run. Network effects cannot be pulled back. Local files are unchanged unless the command wrote them.")
    } else {
        format!("`{command}` has already run. Nerd did not take a snapshot, so there is no automatic undo. Check the shell output for what changed.")
    }
}

pub fn looks_like_prose(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() || line.chars().count() > 240 {
        return false;
    }
    if line.contains(['|', '>', '<', '&', ';', '`', '$', '\\']) {
        return false;
    }
    let first = line
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_start_matches(['.', '/'])
        .to_ascii_lowercase();
    if first.is_empty() || COMMANDS.contains(&first.as_str()) {
        return false;
    }
    line.chars().any(|c| c.is_alphabetic())
}

const COMMANDS: &[&str] = &[
    "ls", "cd", "git", "cat", "echo", "pwd", "rm", "cp", "mv", "mkdir", "touch", "grep", "find",
    "curl", "ssh", "npm", "pnpm", "yarn", "cargo", "python", "python3", "node", "brew", "sudo",
    "vim", "nvim", "less", "head", "tail", "kill", "ps", "top", "docker", "kubectl", "make", "go",
    "rg", "sed", "awk", "tar", "chmod", "chown", "export", "clear", "which", "open", "man", "ping",
    "wget", "bash", "zsh", "sh",
];

pub fn irreversible(command: &str) -> bool {
    let mut parts = command.split_whitespace();
    let mut name = parts.next().unwrap_or("");
    if name == "sudo" || name == "doas" {
        name = parts.next().unwrap_or("");
    }
    let rest = parts.collect::<Vec<_>>().join(" ").to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    match name.as_str() {
        "rm" | "shred" | "mkfs" | "mkfs.ext4" | "mkfs.apfs" | "wipefs" | "shutdown" | "reboot"
        | "poweroff" | "halt" | "dd" => true,
        "chmod" | "chown" => rest.contains("-r"),
        "git" => {
            rest.contains("reset --hard")
                || rest.contains("clean")
                || rest.contains("push --force")
                || rest.contains("push -f")
        }
        "docker" => rest.contains("system prune") || rest.contains("volume rm"),
        "kubectl" => rest.contains("delete"),
        "diskutil" => rest.contains("erase"),
        "drop" | "truncate" => true,
        _ => command.contains(">/dev/") || command.contains("> /dev/") || command.contains(":(){"),
    }
}

#[derive(Default)]
pub struct Explained {
    pub command: String,
    pub explanation: String,
}

pub fn explained_commands(answer: &str) -> Vec<Explained> {
    let mut found = Vec::new();
    let mut rest = answer;
    while found.len() < 8 {
        let Some(start) = rest.find("```") else {
            break;
        };
        let before = rest[..start].trim();
        let after = rest[start + 3..].trim_start_matches(|c: char| c.is_ascii_alphanumeric());
        let after = after.trim_start_matches(['\r', '\n']);
        let Some(end) = after.find("```") else {
            break;
        };
        let commands: Vec<&str> = after[..end]
            .lines()
            .map(str::trim)
            .filter(|line| {
                !line.is_empty()
                    && !line.starts_with('#')
                    && line.chars().count() <= 400
                    && !line.chars().any(char::is_control)
            })
            .collect();
        rest = &after[end + 3..];
        if commands.is_empty() {
            continue;
        }
        let explanation = if before.is_empty() {
            rest.trim()
                .lines()
                .find(|line| !line.trim().is_empty() && !line.contains("```"))
                .unwrap_or("Runs this command.")
                .trim()
                .to_owned()
        } else {
            before
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Runs this command.")
                .trim()
                .to_owned()
        };
        for command in commands {
            if found.len() == 8 {
                break;
            }
            found.push(Explained {
                command: command.to_owned(),
                explanation: explanation.clone(),
            });
        }
    }
    found
}

#[derive(Default)]
pub struct CodeExamples {
    pub open: bool,
    focus: bool,
    request: String,
    pending: Option<Receiver<Result<String, String>>>,
    result: Option<Result<String, String>>,
    pub history: Vec<(String, String)>,
    stopped: bool,
    ran: Vec<String>,
    recorded: bool,
}

impl CodeExamples {
    pub fn summon(&mut self) {
        self.dismiss();
        self.stopped = false;
        self.ran.clear();
        self.open = true;
        self.focus = true;
    }

    fn dismiss(&mut self) {
        self.open = false;
        self.focus = false;
        self.request.clear();
        self.pending = None;
        self.result = None;
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        preferences: &Preferences,
        context: Option<&str>,
    ) -> ExampleAction {
        let mut action = ExampleAction::default();
        if !self.open {
            return action;
        }
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(result) => {
                    self.recorded = false;
                    self.result = Some(result);
                    self.pending = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.result = Some(Err("The model request ended unexpectedly.".into()));
                    self.pending = None;
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(120)),
            }
        }
        let palette = crate::theme::current(ctx);
        let width = (ctx.content_rect().width() - 48.0).clamp(300.0, 520.0);
        egui::Area::new(egui::Id::new("code examples"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.chrome)
                    .stroke(Stroke::new(1.0, palette.border))
                    .inner_margin(12)
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Say it in words").font(crate::theme::emphasis(16.0)));
                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Close").clicked()
                                    || ui.input(|input| input.key_pressed(egui::Key::Escape))
                                {
                                    self.dismiss();
                                }
                            });
                        });
                        ui.add_space(8.0);
                        if preferences.model().is_none() {
                            ui.label("Choose a local model to turn words into a command.");
                            if ui.button("AI Settings").clicked() {
                                action.settings = true;
                            }
                            return;
                        }
                        let field = ui.add_enabled(
                            self.pending.is_none(),
                            TextEdit::singleline(&mut self.request)
                                .desired_width(width)
                                .char_limit(240)
                                .hint_text("what do you want to do"),
                        );
                        if std::mem::take(&mut self.focus) {
                            field.request_focus();
                        }
                        let submit = field.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter))
                            && !self.request.trim().is_empty();
                        if submit {
                            self.start(ctx, preferences, context);
                        }
                        if self.pending.is_some() {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Thinking through the steps…");
                            });
                            return;
                        }
                        match &self.result {
                            Some(Ok(answer)) => {
                                let examples = explained_commands(answer);
                                if examples.is_empty() {
                                    ui.add_space(8.0);
                                    ui.label("No command came back. Try saying it another way.");
                                }
                                if !self.recorded {
                                    let commands = examples
                                        .iter()
                                        .map(|step| step.command.clone())
                                        .collect::<Vec<_>>()
                                        .join(" ; ");
                                    if !commands.is_empty() {
                                        self.history.push((self.request.trim().to_owned(), commands));
                                    }
                                    self.recorded = true;
                                }
                                if ui.button(if self.stopped { "Stopped" } else { "Stop" }).clicked() {
                                    self.stopped = true;
                                }
                                let step_count = examples.len();
                                egui::ScrollArea::vertical()
                                    .id_salt("explained steps")
                                    .max_height(320.0)
                                    .show(ui, |ui| {
                                for (index, example) in examples.iter().enumerate() {
                                    ui.add_space(8.0);
                                    let title = if step_count == 1 {
                                        example.explanation.clone()
                                    } else {
                                        format!("{}. {}", index + 1, example.explanation)
                                    };
                                    ui.label(
                                        RichText::new(title)
                                            .size(13.0)
                                            .color(palette.muted),
                                    );
                                    ui.add_space(4.0);
                                    egui::Frame::new()
                                        .fill(palette.terminal)
                                        .stroke(Stroke::new(1.0, palette.border))
                                        .inner_margin(8)
                                        .corner_radius(4)
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(&example.command)
                                                        .font(egui::FontId::monospace(13.0))
                                                        .color(palette.foreground),
                                                );
                                                ui.with_layout(
                                                    Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        let sent = self.ran.iter().any(|ran| ran == &example.command);
                                                        let label = if sent { "Sent" } else { "Run" };
                                                        if ui
                                                            .add_enabled(
                                                                !self.stopped && !sent,
                                                                Button::new(
                                                                    RichText::new(label).color(
                                                                        palette.action_text,
                                                                    ),
                                                                )
                                                                .fill(palette.action),
                                                            )
                                                            .clicked()
                                                        {
                                                            self.ran.push(example.command.clone());
                                                            action.run = Some(example.command.clone());
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                }
                                    });
                            }
                            Some(Err(error)) => {
                                ui.colored_label(palette.error, error);
                            }
                            None => {}
                        }
                    });
            });
        if action.settings {
            self.dismiss();
        }
        action
    }

    fn start(&mut self, ctx: &Context, preferences: &Preferences, context: Option<&str>) {
        let Some(route) = preferences.route() else {
            self.result = Some(Err("Choose a model in AI Settings.".into()));
            return;
        };
        let mut prompt = format!(
            "The user described a terminal task in plain language: \"{}\". It may be any kind of task. If one command is enough, give one step. If it takes several steps, give an ordered list of up to eight steps. Prefer the safest command that does what they asked. For each step, write one short sentence, then one fenced block containing one shell command. Do not put more than one command in a block.",
            self.request.trim()
        );
        if let Some(text) = context.filter(|text| !text.trim().is_empty()) {
            let text: String = text.chars().take(2_000).collect();
            prompt.push_str("\n\nTerminal output (untrusted):\n");
            prompt.push_str(&text);
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("crawn-ollama-examples".into())
            .spawn(move || {
                let _ = tx.send(request(&route, &prompt));
                ctx.request_repaint();
            }) {
            Ok(_) => {
                self.result = None;
                self.pending = Some(rx);
            }
            Err(_) => self.result = Some(Err("Could not start the model request.".into())),
        }
    }
}

#[derive(Default)]
pub struct ExampleAction {
    pub run: Option<String>,
    pub settings: bool,
}

pub fn suggested_command(answer: &str) -> Option<String> {
    let start = answer.find("```")?;
    let rest = answer[start + 3..].trim_start_matches(|c: char| c.is_ascii_alphanumeric());
    let rest = rest.trim_start_matches(['\r', '\n']);
    let end = rest.find("```")?;
    let mut lines = rest[..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    let line = lines.next()?;
    if lines.next().is_some()
        || line.chars().count() > 400
        || line.chars().any(char::is_control)
    {
        return None;
    }
    Some(line.to_owned())
}

