use super::find::{home_path, SearchKind};
use super::typing::TerminalInput;
use super::widgets::*;
use super::*;

impl TerminalApp {
    pub(super) fn terminal(&mut self, ui: &mut Ui, palette: &Palette, events: &[Event]) {
        let active_id = self.tabs.get(self.active).map(|tab| tab.id);
        for tab in &mut self.tabs {
            if Some(tab.id) != active_id {
                tab.focus(false);
            }
        }
        self.drop_hint = None;
        let mut rect = ui.max_rect();
        if self.tabbed {
            let strip = rect.with_max_y((rect.top() + 36.0).min(rect.bottom()));
            self.tab_strip(ui, palette, strip);
            rect = rect.with_min_y(strip.bottom());
        }
        if self.tabbed {
            if let Some(id) = self.tabs.get(self.active).map(|tab| tab.id) {
                self.paint_pane(ui, palette, events, rect, id);
            }
        } else {
            let layout = self.layout.clone();
            self.paint_node(ui, palette, events, rect, &layout);
        }
        let ctx = ui.ctx().clone();
        let released = ctx.input(|input| input.pointer.any_released());
        if released {
            if let (Some(shell), Some((target, side))) = (self.dragging_pane.take(), self.drop_hint.take()) {
                let split_id = self.next_split;
                self.next_split += 1;
                self.layout.move_beside(shell, target, side, split_id);
            }
        }
        if let Some(request) = self.screen_request.take() {
            match request {
                ScreenRequest::Split { from, side_by_side } => {
                    self.split_from(&ctx, Some(from), side_by_side);
                }
                ScreenRequest::Close(id) => {
                    if let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
                        self.close_tab(&ctx, index);
                    }
                }
            }
        }
    }

    fn paint_node(
        &mut self,
        ui: &mut Ui,
        palette: &Palette,
        events: &[Event],
        rect: egui::Rect,
        node: &Screens,
    ) {
        match node {
            Screens::Shell(id) => self.paint_pane(ui, palette, events, rect, *id),
            Screens::Split {
                id,
                side_by_side,
                ratio,
                first,
                second,
            } => {
                let (first_rect, bar, second_rect) = divide(rect, *side_by_side, *ratio);
                self.paint_node(ui, palette, events, first_rect, first);
                self.paint_node(ui, palette, events, second_rect, second);
                let response = ui.interact(bar, ui.id().with(("screen split", *id)), Sense::drag());
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(if *side_by_side {
                        egui::CursorIcon::ResizeColumn
                    } else {
                        egui::CursorIcon::ResizeRow
                    });
                }
                if let Some(pointer) = response.interact_pointer_pos() {
                    let ratio = ratio_from_pointer(rect, *side_by_side, pointer);
                    self.layout.set_ratio(*id, ratio);
                }
            }
        }
    }

    fn tab_strip(&mut self, ui: &mut Ui, palette: &Palette, rect: egui::Rect) {
        ui.painter().rect_filled(rect, 0.0, palette.chrome);
        let mut bar = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(egui::vec2(8.0, 4.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        let mut chosen = None;
        let mut closed = None;
        for (index, tab) in self.tabs.iter().enumerate() {
            let active = index == self.active;
            let name = elide(&tab.display_name(), 22);
            let tab_button = bar
                .add(Button::new(name).selected(active).min_size(egui::vec2(0.0, 24.0)));
            if tab_button.clicked() {
                chosen = Some(index);
            }
            let close = bar
                .add(Button::new("×").min_size(egui::vec2(22.0, 24.0)))
                .on_hover_text("Close this screen");
            if close.clicked() {
                closed = Some(index);
            }
        }
        if let Some(index) = closed {
            self.screen_request = Some(ScreenRequest::Close(self.tabs[index].id));
        } else if let Some(index) = chosen {
            self.activate(index);
        }
    }

    fn paint_pane(
        &mut self,
        ui: &mut Ui,
        palette: &Palette,
        events: &[Event],
        rect: egui::Rect,
        shell_id: usize,
    ) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == shell_id) else {
            return;
        };
        let active = index == self.active;
        let header_bottom = (rect.top() + 28.0).clamp(rect.top(), rect.bottom());
        let header = rect.with_max_y(header_bottom);
        let body = rect.with_min_y(header.bottom());
        ui.painter().rect_filled(header, 0.0, if active { palette.chrome } else { palette.workspace });
        if active {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(header.left_top(), egui::vec2(2.0, header.height())),
                0.0,
                palette.accent,
            );
        }
        let name = self.tabs[index].display_name();
        let tabbed = self.tabbed;
        let mut bar = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header)
                .layout(Layout::right_to_left(Align::Center)),
        );
        bar.set_clip_rect(header);
        let close = bar
            .add(Button::new(RichText::new("×").size(16.0)).min_size(egui::vec2(26.0, 22.0)))
            .on_hover_text("Close this screen");
        let down = if !tabbed {
            Some(
                bar.add(Button::new(" ").min_size(egui::vec2(26.0, 22.0)))
                    .on_hover_text(shortcut("Split down  ⌘⇧D", "Split down  Ctrl+Alt+D")),
            )
        } else {
            None
        };
        let right = if !tabbed {
            Some(
                bar.add(Button::new(" ").min_size(egui::vec2(26.0, 22.0)))
                    .on_hover_text(shortcut("Split right  ⌘D", "Split right  Ctrl+Shift+D")),
            )
        } else {
            None
        };
        let mode = bar
            .add(Button::new(if tabbed { "Split" } else { "Tabs" }).min_size(egui::vec2(52.0, 22.0)))
            .on_hover_text(if tabbed {
                "Show several shells at once"
            } else {
                "Show one shell at a time"
            });
        if let Some(right) = &right {
            draw_split(&bar, right.rect.center(), true, palette.foreground);
        }
        if let Some(down) = &down {
            draw_split(&bar, down.rect.center(), false, palette.foreground);
        }
        let used = close.rect.width()
            + right.as_ref().map(|r| r.rect.width()).unwrap_or(0.0)
            + down.as_ref().map(|r| r.rect.width()).unwrap_or(0.0)
            + mode.rect.width()
            + 28.0;
        let room = ((header.width() - used) / 7.0).floor().max(4.0) as usize;
        let title = bar.painter().layout_no_wrap(
            elide(&name, room),
            egui::FontId::proportional(12.0),
            if active { palette.foreground } else { palette.muted },
        );
        let handle = bar.allocate_response(
            egui::vec2(bar.available_width().max(24.0), header.height()),
            Sense::click_and_drag(),
        );
        bar.painter().galley(
            egui::pos2(header.left() + 12.0, header.center().y - title.size().y * 0.5),
            title,
            if active { palette.foreground } else { palette.muted },
        );
        if handle.drag_started() && !tabbed {
            self.dragging_pane = Some(shell_id);
            self.activate(index);
        } else if handle.clicked() {
            self.activate(index);
        }
        if self.dragging_pane.is_some() {
            ui.ctx().request_repaint();
        }
        if self.dragging_pane == Some(shell_id) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if handle.hovered() && !tabbed {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            handle.clone().on_hover_text("Drag onto another screen to move it");
        }
        if close.clicked() {
            self.screen_request = Some(ScreenRequest::Close(shell_id));
        } else if down.as_ref().is_some_and(|button| button.clicked()) {
            self.screen_request = Some(ScreenRequest::Split {
                from: shell_id,
                side_by_side: false,
            });
        } else if right.as_ref().is_some_and(|button| button.clicked()) {
            self.screen_request = Some(ScreenRequest::Split {
                from: shell_id,
                side_by_side: true,
            });
        } else if mode.clicked() {
            self.tabbed = !self.tabbed;
        }
        if let Some(pointer) = ui.ctx().input(|input| input.pointer.latest_pos()) {
            if self.dragging_pane.is_some() && self.dragging_pane != Some(shell_id) && rect.contains(pointer) {
                self.drop_hint = Some((shell_id, drop_side(rect, pointer)));
            }
        }
        if let Some((target, side)) = self.drop_hint {
            if target == shell_id {
                ui.painter().rect_filled(drop_zone(rect, side), 6.0, palette.accent.gamma_multiply(0.28));
            }
        }
        let ctx = ui.ctx().clone();
        let grid = Geometry::new(ui, body, self.font_size);
        let response = ui.interact(body, ui.id().with(("shell surface", shell_id)), Sense::click_and_drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Terminal output")
        });
        response.context_menu(|ui| {
            let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == shell_id) else {
                return;
            };
            let selected = tab.selection.is_some();
            if ui.add_enabled(selected, Button::new("Copy")).clicked()
                && let (Some(session), Some(selection)) = (&tab.session, tab.selection)
            {
                let text = session.copy(selection.anchor, selection.end);
                if !text.is_empty() {
                    ui.ctx().copy_text(text);
                }
            }
            if ui.button("Paste").clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste);
            }
            if ui.button("Select all").clicked()
                && let Some(session) = &tab.session
            {
                tab.selection = Some(grid::screen_selection(&session.snapshot()));
            }
        });
        if response.clicked() && self.search.is_some() {
            self.search = None;
            ctx.request_repaint();
        }
        let blocked = egui::Popup::is_any_open(&ctx)
            || self.search.is_some()
            || self.paste.is_some()
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
        if response.clicked() || (self.toolbar_focused && response.gained_focus()) {
            self.toolbar_focused = false;
        }
        if !blocked && !self.toolbar_focused {
            response.request_focus();
            ctx.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            });
        }
        let focused = active && !blocked && !self.toolbar_focused && ctx.input(|i| i.focused);
        if response.clicked() {
            self.activate(index);
        }
        let tab = &mut self.tabs[index];
        tab.focus(focused);
        let mut restart = false;

        let scrolled = if let Some(session) = &tab.session {
            let dims = (grid.cols, grid.rows);
            let scale = ctx.pixels_per_point();
            let pixels = (
                (grid.width * scale).round() as u16,
                (grid.height * scale).round() as u16,
            );
            if (tab.size != dims || tab.cell_pixels != pixels) && !session.exited() {
                match session.resize_with_cell_size(dims.0, dims.1, pixels.0, pixels.1) {
                    Ok(()) => {
                        tab.size = dims;
                        tab.cell_pixels = pixels;
                        tab.selection = None;
                    }
                    Err(error) => {
                        tab.error = Some(format!("Could not resize the shell: {error:#}"))
                    }
                }
            }
            let mut snapshot = session.snapshot();
            let title: String = snapshot
                .title
                .as_deref()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control())
                .take(140)
                .collect();
            if title != tab.title {
                tab.title = title;
                ctx.request_repaint();
            }
            let agent_rect = (focused && snapshot.scrollback_offset == 0 && !snapshot.exited)
                .then_some(snapshot.cursor)
                .flatten()
                .map(|cursor| grid::agent_rect(&grid, cursor));
            let pointer = ui.input(|i| i.pointer.hover_pos());
            let hover = pointer.is_some_and(|pos| body.contains(pos));
            if focused || hover {
                let shell_events: Vec<Event> = pane_events(events, body, focused, self.selecting)
                    .iter()
                    .filter(|event| {
                        let point = match event {
                            Event::PointerButton { pos, .. } | Event::PointerMoved(pos) => {
                                Some(*pos)
                            }
                            _ => None,
                        };
                        !point.is_some_and(|pos| agent_rect.is_some_and(|rect| rect.contains(pos)))
                    })
                    .cloned()
                    .collect();
                let mut input_state = TerminalInput {
                    session: session.as_ref(),
                    selection: &mut tab.selection,
                    composition: &mut self.composition,
                    mouse_button: &mut self.mouse_button,
                    mouse_cell: &mut self.mouse_cell,
                    wheel: &mut self.wheel,
                    selecting: &mut self.selecting,
                    writable: !snapshot.exited,
                    pending_paste: &mut self.paste,
                    focus_cancel: &mut self.focus_cancel,
                    error: &mut tab.error,
                    used: &mut tab.used,
                    draft: &mut self.draft,
                    phrase: &mut self.phrase,
                    can_convert: self.ai_preferences.model().is_some() && self.phrase_job.is_none(),
                };
                if input_state.process(&ctx, &shell_events, &grid, pointer, &snapshot) {
                    snapshot = session.snapshot();
                }
            }
            let found = self
                .search
                .as_ref()
                .and_then(|search| search.matches.get(search.index))
                .filter(|found| found.scrollback_offset == snapshot.scrollback_offset)
                .map(|found| (found.row, found.col, found.len));
            snapshot.cursor_shape = self.cursor_style.resolve(snapshot.cursor_shape);
            grid::paint(
                &ui.painter_at(body),
                &grid,
                &snapshot,
                palette,
                tab.selection,
                found,
                focused,
            );
            if focused && !snapshot.exited {
                let (row, col) = snapshot.cursor.unwrap_or((0, 0));
                let pos =
                    grid.rect.min + egui::vec2(col as f32 * grid.width, row as f32 * grid.height);
                let cursor_rect = egui::Rect::from_min_size(pos, egui::vec2(1.0, grid.height));
                if snapshot.scrollback_offset == 0
                    && let Some(mark) = snapshot.cursor.map(|cursor| grid::agent_rect(&grid, cursor))
                {
                    let response = ui.interact(mark, ui.id().with(("ai agent", shell_id)), Sense::click());
                    grid::paint_agent(
                        &ui.painter_at(mark),
                        mark,
                        palette.accent,
                        ctx.input(|input| input.time),
                    );
                    let response = response.on_hover_text("Describe a command");
                    if response.hovered() || response.clicked() {
                        ctx.request_repaint_after(std::time::Duration::from_millis(50));
                    } else {
                        ctx.request_repaint_after(std::time::Duration::from_millis(90));
                    }
                    if response.clicked() {
                        self.examples.summon();
                    }
                }
                if self.phrase.is_some() || self.phrase_job.is_some() {
                    let galley = ui.painter().layout_no_wrap(
                        "asking AI…".to_owned(),
                        egui::FontId::proportional(12.0),
                        palette.muted,
                    );
                    let label = pos
                        + egui::vec2(grid.width + 42.0, (grid.height - galley.size().y) * 0.5);
                    ui.painter().galley(label, galley, palette.muted);
                    ctx.request_repaint_after(std::time::Duration::from_millis(120));
                }
                ctx.output_mut(|output| {
                    output.ime = Some(egui::output::IMEOutput {
                        rect: cursor_rect,
                        cursor_rect,
                    })
                });
                if !self.composition.is_empty() {
                    let galley = ui.painter().layout_no_wrap(
                        self.composition.clone(),
                        grid.font.clone(),
                        palette.foreground,
                    );
                    let preedit = egui::Rect::from_min_size(pos, galley.size());
                    ui.painter().rect_filled(preedit, 2, palette.accent_surface);
                    ui.painter().galley(pos, galley, palette.foreground);
                }
            }
            if self.watch_failure {
                if let Some(line) = labs::failure_line(&grid::plain_text(&snapshot)) {
                    self.failure = Some(format!(
                        "{line}\nA safer retry is to run the command with --help, or print the paths before changing anything."
                    ));
                    self.watch_failure = false;
                }
            }
            if snapshot.exited {
                restart = exit_card(ui, body, palette, snapshot.exit_code);
            }
            snapshot.scrollback_offset > 0
        } else {
            restart = retry_card(ui, body, palette, tab.error.as_deref());
            false
        };
        if tab.scrolled != scrolled {
            tab.scrolled = scrolled;
            ctx.request_repaint();
        }

            if restart {
            let (id, size) = (tab.id, tab.size);
            let custom_title = tab.custom_title.take();
            self.tabs[index] = self.make_tab(&ctx, id, size);
            self.tabs[index].custom_title = custom_title;
            self.active = index;
            self.reset_interaction();
        }
    }

    pub(super) fn status_overlay(
        &mut self,
        ctx: &Context,
        palette: &Palette,
        sidebar: bool,
    ) -> Option<egui::Rect> {
        let tab = &mut self.tabs[self.active];
        if let Some(error) = tab.error.as_deref().filter(|_| tab.session.is_some()) {
            let left = if sidebar { SIDEBAR_WIDTH + 26.0 } else { 14.0 };
            egui::Area::new(egui::Id::new("shell error"))
                .anchor(egui::Align2::LEFT_BOTTOM, [left, -12.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    Frame::new()
                        .fill(palette.chrome)
                        .corner_radius(6)
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.label(RichText::new(elide(error, 58)).color(palette.error))
                                .on_hover_text(error);
                        });
                });
        }
        if !tab.scrolled {
            return None;
        }
        let nudge = egui::Area::new(egui::Id::new("return to latest"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-14.0, -12.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| ui.button("Jump to Latest"));
        if nudge.inner.clicked() {
            if let Some(session) = &tab.session {
                session.scroll_to_bottom();
            }
            tab.selection = None;
        }
        Some(nudge.response.rect)
    }

    pub(super) fn modal(&mut self, ctx: &Context) {
        if let Some(name) = &mut self.rename {
            let response = Modal::new(egui::Id::new("rename session")).show(ctx, |ui| {
                ui.set_min_width(300.0);
                ui.heading("Rename session");
                ui.label("Leave the name blank to use the shell title.");
                let field = ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text("Session name")
                        .char_limit(64),
                );
                if std::mem::take(&mut self.focus_rename) {
                    field.request_focus();
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let save = ui.button("Save").clicked()
                        || (field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                    (save, ui.button("Cancel").clicked())
                })
                .inner
            });
            if response.inner.0 {
                let title: String = name.trim().chars().filter(|c| !c.is_control()).collect();
                self.tabs[self.active].custom_title = (!title.is_empty()).then_some(title);
                self.rename = None;
            } else if response.inner.1 || response.should_close() {
                self.rename = None;
            }
        }
        if let Some(action) = self.commands.show(ctx) {
            self.action(ctx, action);
        }
        let tab = &self.tabs[self.active];
        let assistant = self.assistant.show(ctx, &self.ai_preferences, || {
            let session = tab.session.as_ref()?;
            Some(if let Some(selection) = tab.selection {
                session.copy(selection.anchor, selection.end)
            } else {
                grid::plain_text(&session.snapshot())
            })
        });
        if assistant.open_settings {
            self.settings_page = SettingsPage::Ai;
            self.settings_open = true;
        }
        if let Some(command) = assistant.insert_command {
            self.insert_command(command);
        }
        let context = {
            let tab = &self.tabs[self.active];
            tab.session.as_ref().map(|session| {
                if let Some(selection) = tab.selection {
                    session.copy(selection.anchor, selection.end)
                } else {
                    grid::plain_text(&session.snapshot())
                }
            })
        };
        let prompt = self
            .prompt
            .show(ctx, &self.ai_preferences, context.as_deref());
        if let Some(command) = prompt.insert {
            self.insert_command(command);
        }
        if prompt.open_assistant {
            self.assistant.summon();
        }
        if prompt.open_settings {
            self.settings_page = SettingsPage::Ai;
            self.settings_open = true;
        }
        let example_context = context.clone();
        let examples = self
            .examples
            .show(ctx, &self.ai_preferences, example_context.as_deref());
        if let Some(command) = examples.run {
            if let Some(reason) = ai::gate(&command) {
                self.pending_reason = Some(reason);
                self.pending_run = Some(command);
            } else {
                self.finish_run(command);
            }
        }
        if let Some(command) = self.pending_run.clone() {
            let palette = theme::current(ctx);
            let mut decision = None;
            egui::Area::new(egui::Id::new("confirm run"))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(palette.warning_surface)
                        .stroke(Stroke::new(1.0, palette.border))
                        .inner_margin(12)
                        .corner_radius(4)
                        .show(ui, |ui| {
                            ui.set_min_width(320.0);
                            ui.label(
                                RichText::new(
                                    self.pending_reason
                                        .as_deref()
                                        .unwrap_or("are you sure?"),
                                )
                                .font(egui::FontId::monospace(13.0))
                                .color(palette.foreground),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("$ {command}"))
                                    .font(egui::FontId::monospace(13.0))
                                    .color(palette.foreground),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("Yes, run it").clicked() {
                                    decision = Some(true);
                                }
                                if ui.button("No").clicked()
                                    || ui.input(|input| input.key_pressed(Key::Escape))
                                {
                                    decision = Some(false);
                                }
                            });
                        });
                });
            match decision {
                Some(true) => {
                    self.pending_run = None;
                    self.pending_reason = None;
                    self.finish_run(command);
                }
                Some(false) => {
                    self.pending_run = None;
                    self.pending_reason = None;
                }
                None => {}
            }
        }
        if examples.settings {
            self.settings_page = SettingsPage::Ai;
            self.settings_open = true;
        }
        if self.settings_open {
            let response = Modal::new(egui::Id::new("settings modal")).show(ctx, |ui| {
                self.settings(ui);
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.button("Done").clicked()
                })
                .inner
            });
            if response.inner || response.should_close() {
                self.settings_open = false;
            }
        }
        if self.file_search.open {
            let mut chosen = None;
            let mut close = false;
            self.file_search.poll();
            let _response = Modal::new(egui::Id::new("find path")).show(ctx, |ui| {
                ui.set_width(480.0);
                ui.set_min_height(360.0);
                ui.heading("Find a file or folder");
                ui.horizontal(|ui| {
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.file_search.query)
                            .hint_text("Name")
                            .desired_width(330.0),
                    );
                    if std::mem::take(&mut self.file_search.focus) {
                        field.request_focus();
                    }
                    let mut kind = self.file_search.kind;
                    egui::ComboBox::from_id_salt("find kind")
                        .width(110.0)
                        .selected_text(kind.label())
                        .show_ui(ui, |ui| {
                            for choice in [SearchKind::All, SearchKind::Files, SearchKind::Folders] {
                                ui.selectable_value(&mut kind, choice, choice.label());
                            }
                        });
                    self.file_search.kind = kind;
                    self.file_search.sync(ctx);
                });
                ui.add_space(6.0);
                if self.file_search.pending {
                    ui.spinner();
                    ui.label("Searching…");
                } else if self.file_search.query.trim().chars().count() >= 2 {
                    ui.label(format!("{} found", self.file_search.hits.len()));
                }
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .min_scrolled_height(240.0)
                    .show(ui, |ui| {
                        ui.set_min_height(240.0);
                        if self.file_search.query.trim().chars().count() < 2 {
                            ui.label("Type at least two letters. ↑ and ↓ move, Enter inserts the path.");
                        } else if self.file_search.hits.is_empty() && !self.file_search.pending {
                            ui.label("Nothing matches that name.");
                        }
                        let count = self.file_search.hits.len();
                        if count > 0 {
                            self.file_search.selected = self.file_search.selected.min(count - 1);
                        }
                        for (index, path) in self.file_search.hits.iter().enumerate() {
                            let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
                            let parent = home_path(path.trim_end_matches(name).trim_end_matches(['/', '\\']));
                            let kind = if std::path::Path::new(path).is_dir() { "Folder" } else { "File" };
                            let label = if parent.is_empty() {
                                format!("{name}  ·  {kind}")
                            } else {
                                format!("{name}  ·  {kind}\n{parent}")
                            };
                            let response = ui.selectable_label(index == self.file_search.selected, label);
                            response.context_menu(|ui| {
                                if ui.button("Copy path").clicked() {
                                    ui.ctx().copy_text(path.clone());
                                }
                            });
                            if response.hovered() {
                                self.file_search.selected = index;
                            }
                            if response.on_hover_text(path).clicked() {
                                chosen = Some(path.clone());
                            }
                        }
                        let move_down = ui.input(|input| input.key_pressed(Key::ArrowDown));
                        let move_up = ui.input(|input| input.key_pressed(Key::ArrowUp));
                        let accept = ui.input(|input| input.key_pressed(Key::Enter));
                        if move_down && count > 0 {
                            self.file_search.selected = (self.file_search.selected + 1).min(count - 1);
                        } else if move_up && self.file_search.selected > 0 {
                            self.file_search.selected -= 1;
                        } else if accept && count > 0 {
                            chosen = Some(self.file_search.hits[self.file_search.selected].clone());
                        }
                    });
                ui.add_space(6.0);
                close = ui.button("Close").clicked() || ui.input(|input| input.key_pressed(Key::Escape));
            });
            if let Some(path) = chosen {
                self.insert_command(shell_quote(&path));
                self.file_search.open = false;
            } else if close {
                self.file_search.open = false;
            }
        }
        if self.shortcuts_open {
            let response = Modal::new(egui::Id::new("shortcuts modal")).show(ctx, |ui| {
                ui.set_min_width(300.0);
                ui.heading("Keyboard shortcuts");
                ui.add_space(8.0);
                shortcut_list(ui);
                ui.add_space(10.0);
                ui.button("Done").clicked()
            });
            if response.inner || response.should_close() {
                self.shortcuts_open = false;
            }
        }
        if let Some(text) = &self.read_output {
            let focus_output = std::mem::take(&mut self.focus_output);
            let response = Modal::new(egui::Id::new("visible output modal")).show(ctx, |ui| {
                let width = (ctx.content_rect().width() - 48.0).clamp(260.0, 610.0);
                ui.set_width(width);
                ui.heading("Visible Terminal Output");
                ui.add_space(4.0);
                let height = (ctx.content_rect().height() - 126.0).clamp(60.0, 330.0);
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .show(ui, |ui| {
                        let mut output = text.as_str();
                        let field = ui.add(
                            egui::TextEdit::multiline(&mut output)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(width)
                                .hint_text("No visible output"),
                        );
                        field.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::TextEdit,
                                true,
                                "Read-only terminal output",
                            )
                        });
                        if focus_output {
                            field.request_focus();
                        }
                    });
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let done = ui.button("Done").clicked();
                    if ui
                        .add_enabled(!text.is_empty(), Button::new("Copy Output"))
                        .clicked()
                    {
                        ctx.copy_text(text.clone());
                    }
                    done
                })
                .inner
            });
            if response.inner || response.should_close() {
                self.read_output = None;
            }
        }
        if let Some(text) = &self.paste {
            let pasted = text.clone();
            self.labs
                .explain_paste(ctx, &self.ai_preferences, &pasted);
            self.labs.poll_paste();
            let note = self.labs.paste_note.clone();
            let tab = elide(&self.tabs[self.active].display_name(), 26);
            let focus_cancel = std::mem::take(&mut self.focus_cancel);
            let response = Modal::new(egui::Id::new("paste modal")).show(ctx, |ui| {
                ui.set_width(360.0);
                ui.heading(format!("Paste lines into {tab}?"));
                ui.label("Line breaks can run commands in this shell. Nothing is sent until you press Paste Lines.");
                if let Some(note) = &note {
                    ui.add_space(4.0);
                    ui.label(note);
                } else {
                    ui.label("Reading each line…");
                }
                ui.add_space(5.0);
                let preview: String = text.chars().take(1200).collect();
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        ui.monospace(&preview);
                        if preview.len() < text.len() {
                            ui.label("Preview shortened");
                        }
                    });
                ui.add_space(7.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let paste = ui.button("Paste Lines");
                    let cancel = ui.button("Cancel");
                    if focus_cancel {
                        cancel.request_focus();
                    }
                    (cancel.clicked(), paste.clicked())
                })
                .inner
            });
            if response.inner.1 {
                self.labs.paste_note = None;
                self.labs.paste_job = None;
                if let (Some(session), Some(text)) =
                    (&self.tabs[self.active].session, self.paste.take())
                {
                    match session.paste(&text) {
                        Ok(()) => {
                            session.scroll_to_bottom();
                            self.tabs[self.active].selection = None;
                        }
                        Err(error) => {
                            self.tabs[self.active].error =
                                Some(format!("Could not paste: {error:#}"))
                        }
                    }
                }
            } else if response.inner.0 || response.should_close() {
                self.paste = None;
                self.labs.paste_note = None;
                self.labs.paste_job = None;
            }
        }
        if self.ai_preferences.wizard {
            let response = Modal::new(egui::Id::new("model wizard")).show(ctx, |ui| {
                self.ai_preferences.wizard(ui)
            });
            if response.inner {
                // Saved from the last step. Persistence happens with the rest of the settings.
            }
            if response.should_close() {
                self.ai_preferences.wizard = false;
            }
        }
        if self.labs.open {
            let sessions: Vec<(String, String)> = self
                .tabs
                .iter()
                .map(|tab| {
                    let text = tab
                        .session
                        .as_ref()
                        .map(|session| grid::plain_text(&session.snapshot()))
                        .unwrap_or_default();
                    (tab.display_name(), text)
                })
                .collect();
            let history = self.examples.history.clone();
            let undo = self.last_undo.clone();
            let failure = self.failure.clone();
            let response = Modal::new(egui::Id::new("labs")).show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 48.0).clamp(320.0, 560.0));
                self.labs.show(
                    ui,
                    &self.ai_preferences,
                    &sessions,
                    &history,
                    undo.as_deref(),
                    failure.as_deref(),
                );
                ui.add_space(8.0);
                ui.button("Done").clicked()
            });
            if response.inner || response.should_close() {
                self.labs.open = false;
            }
        }
    }
}

fn divide(rect: egui::Rect, side_by_side: bool, ratio: f32) -> (egui::Rect, egui::Rect, egui::Rect) {
    let gap = 8.0;
    let ratio = if side_by_side {
        fit_ratio(rect.width(), ratio, 160.0)
    } else {
        fit_ratio(rect.height(), ratio, 120.0)
    };
    if side_by_side {
        let x = rect.left() + rect.width() * ratio;
        (
            rect.with_max_x(x - gap / 2.0),
            egui::Rect::from_min_max(egui::pos2(x - gap / 2.0, rect.top()), egui::pos2(x + gap / 2.0, rect.bottom())),
            rect.with_min_x(x + gap / 2.0),
        )
    } else {
        let y = rect.top() + rect.height() * ratio;
        (
            rect.with_max_y(y - gap / 2.0),
            egui::Rect::from_min_max(egui::pos2(rect.left(), y - gap / 2.0), egui::pos2(rect.right(), y + gap / 2.0)),
            rect.with_min_y(y + gap / 2.0),
        )
    }
}

fn ratio_from_pointer(rect: egui::Rect, side_by_side: bool, pointer: egui::Pos2) -> f32 {
    let (span, offset) = if side_by_side {
        (rect.width(), pointer.x - rect.left())
    } else {
        (rect.height(), pointer.y - rect.top())
    };
    fit_ratio(span, offset / span.max(1.0), if side_by_side { 160.0 } else { 120.0 })
}

fn fit_ratio(span: f32, ratio: f32, min_px: f32) -> f32 {
    if span <= min_px * 2.0 {
        0.5
    } else {
        let min = min_px / span;
        ratio.clamp(min, 1.0 - min)
    }
}

fn pane_events(events: &[Event], body: egui::Rect, keys: bool, selecting: bool) -> Vec<Event> {
    events
        .iter()
        .filter(|event| match event {
            Event::Key { .. } | Event::Text(_) | Event::Ime(_) => keys,
            Event::PointerButton { pos, pressed, .. } => body.contains(*pos) || (!pressed && keys),
            Event::PointerMoved(pos) => body.contains(*pos) || (keys && selecting),
            Event::MouseWheel { .. } => true,
            _ => false,
        })
        .cloned()
        .collect()
}

fn drop_side(rect: egui::Rect, pointer: egui::Pos2) -> DropSide {
    let x = (pointer.x - rect.left()) / rect.width().max(1.0);
    let y = (pointer.y - rect.top()) / rect.height().max(1.0);
    if (x - 0.5).abs() > (y - 0.5).abs() {
        if x < 0.5 { DropSide::Left } else { DropSide::Right }
    } else if y < 0.5 {
        DropSide::Above
    } else {
        DropSide::Below
    }
}

fn drop_zone(rect: egui::Rect, side: DropSide) -> egui::Rect {
    match side {
        DropSide::Left => rect.with_max_x(rect.center().x),
        DropSide::Right => rect.with_min_x(rect.center().x),
        DropSide::Above => rect.with_max_y(rect.center().y),
        DropSide::Below => rect.with_min_y(rect.center().y),
    }
}

fn draw_split(ui: &Ui, center: egui::Pos2, side_by_side: bool, color: egui::Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.2, color);
    painter.rect_stroke(
        egui::Rect::from_center_size(center, egui::vec2(13.0, 11.0)),
        1.5,
        stroke,
        egui::StrokeKind::Inside,
    );
    if side_by_side {
        painter.line_segment([center + egui::vec2(0.0, -5.0), center + egui::vec2(0.0, 5.0)], stroke);
    } else {
        painter.line_segment([center + egui::vec2(-6.0, 0.0), center + egui::vec2(6.0, 0.0)], stroke);
    }
}
