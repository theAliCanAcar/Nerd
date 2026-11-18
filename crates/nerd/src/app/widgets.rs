use super::*;

pub(super) fn setting_title(ui: &mut Ui, title: &str, detail: &str) {
    ui.label(RichText::new(title).font(theme::emphasis(14.0)));
    ui.label(RichText::new(detail).weak());
    ui.add_space(4.0);
}

pub(super) fn shortcut_list(ui: &mut Ui) {
    egui::Grid::new("shortcut keys")
        .spacing([32.0, 9.0])
        .show(ui, |ui| {
            for (action, mac, other) in [
                ("New tab", "⌘T", "Ctrl+Shift+T"),
                ("Split right", "⌘D", "Ctrl+Shift+D"),
                ("Split down", "⌘⇧D", "Ctrl+Alt+D"),
                ("Close tab", "⌘W", "Ctrl+Shift+W"),
                ("Previous tab", "⌘⇧[", "Ctrl+Page Up"),
                ("Next tab", "⌘⇧]", "Ctrl+Page Down"),
                ("Find", "⌘F", "Ctrl+Shift+F"),
                ("Command Palette", "⇧⌘P", "Ctrl+Shift+P"),
                ("Ask AI", "Shift+Space", "Shift+Space"),
                ("Find a file or folder", "⇧⌘Space", "Ctrl+Shift+Space"),
                ("Ask AI from the menu", "⇧⌘I", "Ctrl+Shift+I"),
                ("Toggle sidebar", "Option+⌘S", "Ctrl+Shift+B"),
                ("Switch toolbar / shell focus", "⌘⇧F6", "Ctrl+Shift+F6"),
                ("Copy selection", "⌘C", "Ctrl+Shift+C"),
                ("Select all visible text", "⌘A", "Ctrl+Shift+A"),
                ("Paste", "⌘V", "Ctrl+Shift+V"),
                (
                    "Increase / decrease text size",
                    "⌘+ / ⌘−",
                    "Ctrl+Plus / Ctrl+Minus",
                ),
                ("Reset text size", "⌘0", "Ctrl+0"),
                (
                    "Scroll by page",
                    "Shift+Page Up / Down",
                    "Shift+Page Up / Down",
                ),
            ] {
                ui.label(action);
                ui.monospace(shortcut(mac, other));
                ui.end_row();
            }
        });
}

pub(super) fn icon_button(ui: &mut Ui, symbol: &str, label: &str, enabled: bool) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        Button::new(symbol)
            .frame_when_inactive(false)
            .min_size(egui::vec2(28.0, 28.0)),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response.on_hover_text(label)
}

pub(super) fn chrome_frame(palette: &Palette, margin: Margin) -> Frame {
    Frame::new()
        .fill(palette.chrome)
        .inner_margin(margin)
        .stroke(Stroke::new(1.0, palette.border))
}

pub(super) fn exit_card(ui: &mut Ui, rect: egui::Rect, palette: &Palette, code: Option<i32>) -> bool {
    let detail = code.map_or_else(
        || "The session has ended.".to_owned(),
        |code| format!("Exit status {code}"),
    );
    terminal_card(ui, rect, palette, "Shell exited", &detail, "Restart shell")
}

pub(super) fn retry_card(ui: &mut Ui, rect: egui::Rect, palette: &Palette, error: Option<&str>) -> bool {
    terminal_card(
        ui,
        rect,
        palette,
        "Could not start the shell",
        error.unwrap_or("The session could not be opened."),
        "Try again",
    )
}

pub(super) fn terminal_card(
    ui: &mut Ui,
    rect: egui::Rect,
    palette: &Palette,
    title: &str,
    detail: &str,
    action: &str,
) -> bool {
    let width = (rect.width() - 30.0).min(360.0);
    let position = egui::pos2(rect.center().x - width / 2.0, rect.center().y - 46.0);
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(position, Vec2::new(width, 92.0))),
    );
    Frame::new()
        .fill(palette.chrome)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(8)
        .inner_margin(14)
        .show(&mut ui, |ui| {
            ui.set_min_width(width - 30.0);
            ui.label(RichText::new(title).size(16.0).strong());
            ui.label(RichText::new(detail).color(palette.muted));
            ui.add_space(4.0);
            ui.button(action).clicked()
        })
        .inner
}


pub(super) fn shell_quote(path: &str) -> String {
    if path.contains(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | '\\')) {
        format!("'{}'", path.replace('\'', "'\\''"))
    } else {
        path.to_owned()
    }
}

pub(super) fn shortcut<'a>(mac: &'a str, other: &'a str) -> &'a str {
    if MAC { mac } else { other }
}

pub(super) fn elide(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let mut short: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        short.push('…');
    }
    short
}
