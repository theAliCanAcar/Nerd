use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui::{self, Context, RichText, ScrollArea, TextEdit, Ui};

use crate::ai::{self, Preferences};
use crate::theme;

pub fn output_diff(left: &str, right: &str) -> String {
    let right_lines: HashSet<&str> = right.lines().filter(|line| !line.trim().is_empty()).collect();
    let left_lines: HashSet<&str> = left.lines().filter(|line| !line.trim().is_empty()).collect();
    let mut report = String::from("Lines only in the first session:\n");
    push_unique(&mut report, left.lines().filter(|line| !right_lines.contains(line)));
    report.push_str("\nLines only in the second session:\n");
    push_unique(&mut report, right.lines().filter(|line| !left_lines.contains(line)));
    report
}

fn push_unique<'a>(report: &mut String, lines: impl Iterator<Item = &'a str>) {
    let mut count = 0;
    for line in lines.map(str::trim).filter(|line| !line.is_empty()) {
        if count == 30 {
            report.push_str("…\n");
            break;
        }
        report.push_str(line);
        report.push('\n');
        count += 1;
    }
    if count == 0 {
        report.push_str("(none)\n");
    }
}

pub fn handoff(history: &[(String, String)], output: &str) -> String {
    let mut note = String::from("# Session handoff\n\n## What you asked\n");
    if history.is_empty() {
        note.push_str("No plain-language commands yet.\n");
    }
    for (words, command) in history.iter().rev().take(8) {
        note.push_str(&format!("- {words} → {command}\n"));
    }
    note.push_str("\n## Recent output\n```\n");
    let tail: String = output.chars().rev().take(1200).collect::<String>().chars().rev().collect();
    note.push_str(tail.trim());
    note.push_str("\n```\n\n## Next step\nContinue from the current prompt. Nerd did not change files while writing this note.\n");
    note
}

pub fn failure_line(output: &str) -> Option<String> {
    output.lines().rev().take(25).find_map(|line| {
        let lower = line.to_ascii_lowercase();
        if lower.contains("command not found")
            || lower.contains("no such file")
            || lower.contains("permission denied")
            || lower.contains("error:")
            || lower.contains("failed")
        {
            Some(line.trim().chars().take(180).collect())
        } else {
            None
        }
    })
}

#[derive(Default)]
pub struct Labs {
    pub open: bool,
    meaning: String,
    meaning_job: Option<Receiver<Result<String, String>>>,
    meaning_answer: Option<String>,
    diff_left: usize,
    diff_right: usize,
    pub paste_job: Option<Receiver<Result<String, String>>>,
    pub paste_note: Option<String>,
}

impl Labs {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        preferences: &Preferences,
        sessions: &[(String, String)],
        history: &[(String, String)],
        undo: Option<&str>,
        failure: Option<&str>,
    ) {
        ui.heading("Tools");
        ui.label("These tools read the current terminal. They do not run commands or change files.");
        ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            section(ui, "Plain-language history");
            if history.is_empty() {
                ui.label("Words you send from the star box appear here with the command they became.");
            }
            for (words, command) in history.iter().rev().take(8) {
                ui.label(RichText::new(format!("{words} → {command}")).font(egui::FontId::monospace(12.0)));
            }
            section(ui, "Search scrollback by meaning");
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut self.meaning).hint_text("when did the migration fail?").desired_width(220.0));
                if ui.button("Search").clicked() {
                    self.start_meaning(ui.ctx(), preferences, sessions.first().map(|(_, text)| text.as_str()));
                }
            });
            poll_text(&mut self.meaning_job, &mut self.meaning_answer);
            if let Some(answer) = &self.meaning_answer {
                ui.label(answer);
            }
            section(ui, "Commands");
            ui.label("Nerd does not replay a command in a side folder. A temporary directory would not contain the command, and deleting that folder would not undo it.");
            section(ui, "Two sessions");
            if sessions.len() >= 2 {
                ui.horizontal(|ui| {
                    pick(ui, "diff left", &mut self.diff_left, sessions);
                    pick(ui, "diff right", &mut self.diff_right, sessions);
                });
                let left = sessions.get(self.diff_left).map(|(_, text)| text.as_str()).unwrap_or("");
                let right = sessions.get(self.diff_right).map(|(_, text)| text.as_str()).unwrap_or("");
                ui.monospace(output_diff(left, right));
            } else {
                ui.label("Open a second used session to compare output.");
            }
            section(ui, "Honest undo");
            ui.label(undo.unwrap_or("Run a command from the star box to see what Nerd can and cannot restore."));
            section(ui, "Failure");
            ui.label(failure.unwrap_or("No recent failing line was seen. A safer retry is to read the command with --help before running it again."));
            section(ui, "Session handoff");
            let note = handoff(history, sessions.first().map(|(_, text)| text.as_str()).unwrap_or(""));
            if ui.button("Copy handoff").clicked() {
                ui.ctx().copy_text(note.clone());
            }
            ui.monospace(&note);
        });
    }

    pub fn poll_paste(&mut self) {
        if let Some(job) = &self.paste_job {
            match job.try_recv() {
                Ok(Ok(text)) => {
                    self.paste_note = Some(text);
                    self.paste_job = None;
                }
                Ok(Err(error)) => {
                    self.paste_note = Some(error);
                    self.paste_job = None;
                }
                Err(TryRecvError::Disconnected) => self.paste_job = None,
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    pub fn explain_paste(&mut self, ctx: &Context, preferences: &Preferences, text: &str) {
        if self.paste_job.is_some() || self.paste_note.is_some() {
            return;
        }
        let Some(route) = preferences.route() else {
            self.paste_note = Some("Each line is shown below and is not sent until you press Paste Lines.".into());
            return;
        };
        let prompt = format!(
            "Explain this pasted shell text one line at a time. Do not suggest running it. Say which lines can change files or use the network.\n\n{text}"
        );
        self.paste_job = Some(spawn_model(ctx, route, prompt));
    }

    fn start_meaning(&mut self, ctx: &Context, preferences: &Preferences, output: Option<&str>) {
        let Some(route) = preferences.route() else {
            self.meaning_answer = Some("Choose a model in AI Settings.".into());
            return;
        };
        let output = output.unwrap_or("").chars().take(4000).collect::<String>();
        let prompt = format!(
            "The user is searching terminal scrollback by meaning: \"{}\". Quote the matching lines. If nothing matches, say that nothing matches. Do not invent commands.\n\n{output}",
            self.meaning.trim()
        );
        self.meaning_answer = None;
        self.meaning_job = Some(spawn_model(ctx, route, prompt));
    }
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).font(theme::emphasis(14.0)));
}

fn pick(ui: &mut Ui, id: &str, selected: &mut usize, sessions: &[(String, String)]) {
    let label = sessions.get(*selected).map(|(name, _)| name.as_str()).unwrap_or("Session");
    egui::ComboBox::from_id_salt(id).selected_text(label).show_ui(ui, |ui| {
        for (index, (name, _)) in sessions.iter().enumerate() {
            ui.selectable_value(selected, index, name);
        }
    });
}

fn poll_text(job: &mut Option<Receiver<Result<String, String>>>, slot: &mut Option<String>) {
    let ready = job.as_ref().and_then(|job| match job.try_recv() {
        Ok(result) => Some(result.unwrap_or_else(|error| error)),
        Err(TryRecvError::Disconnected) => Some("The request stopped.".into()),
        Err(TryRecvError::Empty) => None,
    });
    if let Some(text) = ready {
        *job = None;
        *slot = Some(text);
    }
}

fn spawn_model(ctx: &Context, route: ai::Route, prompt: String) -> Receiver<Result<String, String>> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    let _ = std::thread::Builder::new().name("nerd-labs".into()).spawn(move || {
        let _ = tx.send(ai::request(&route, &prompt));
        ctx.request_repaint();
    });
    rx
}
