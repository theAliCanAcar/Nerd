use super::widgets::*;
use super::*;

impl TerminalApp {
    pub(super) fn sidebar(&mut self, ctx: &Context, palette: &Palette) {
        let reveal_session = std::mem::take(&mut self.reveal_session);
        let mut activate = None;
        let mut close = None;
        egui::SidePanel::left("sessions sidebar")
            .exact_width(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(Frame::new().fill(palette.sidebar).inner_margin(13))
            .show(ctx, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (mark, _) = ui.allocate_exact_size(egui::vec2(29.0, 29.0), Sense::hover());
                    ui.painter().rect_filled(mark, 4, palette.accent_surface);
                    chrome::draw_glasses(ui.painter(), mark.center(), 0.85, palette.accent);
                    ui.label(RichText::new("Nerd").font(theme::emphasis(22.0)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let credit = ui.add(
                            egui::Label::new(RichText::new("by crawn.ai").size(11.0).color(palette.muted))
                                .sense(Sense::click()),
                        );
                        if credit.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if credit.clicked() {
                            ai::open_link("https://crawn.ai");
                        }
                    });
                });
                ui.add_space(17.0);
                let new = ui
                    .scope(|ui| {
                        ui.visuals_mut().override_text_color = Some(palette.accent);
                        ui.add_sized(
                            [ui.available_width(), 36.0],
                            Button::new(RichText::new("New session").strong())
                                .corner_radius(4)
                                .fill(palette.accent_surface)
                                .stroke(Stroke::new(
                                    1.0,
                                    palette.border.lerp_to_gamma(palette.accent, 0.25),
                                )),
                        )
                    })
                    .inner;
                chrome::draw_icon(
                    ui,
                    egui::pos2(new.rect.left() + 19.0, new.rect.center().y),
                    Icon::Plus,
                    palette.accent,
                );
                if new.on_hover_text(shortcut("⌘T", "Ctrl+Shift+T")).clicked() {
                    self.new_tab(ctx);
                }
                ui.add_space(17.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Sessions")
                            .size(12.0)
                            .strong()
                            .color(palette.muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = if self.tabbed { "Split" } else { "Tabs" };
                        if ui
                            .small_button(label)
                            .on_hover_text(if self.tabbed {
                                "Show several shells at once"
                            } else {
                                "Show one shell at a time"
                            })
                            .clicked()
                        {
                            self.tabbed = !self.tabbed;
                        }
                    });
                });
                if self.host.is_some() {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Closing the window leaves these shells running.")
                            .size(11.0)
                            .color(palette.muted),
                    );
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_salt("session rows")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (index, tab) in self.tabs.iter().enumerate() {
                            let active = index == self.active;
                            let title = tab.display_name();
                            ui.push_id(tab.id, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    let item = chrome::control(
                                        ui,
                                        palette,
                                        ui.available_width() - 36.0,
                                        Icon::Terminal,
                                        &title,
                                        Some(active),
                                    );
                                    if active && reveal_session {
                                        item.scroll_to_me_animation(
                                            None,
                                            egui::style::ScrollAnimation::none(),
                                        );
                                    }
                                    if item.clicked() {
                                        activate = Some(index);
                                    }
                                    if chrome::control(
                                        ui,
                                        palette,
                                        34.0,
                                        Icon::Close,
                                        &format!("Close {title}"),
                                        None,
                                    )
                                    .clicked()
                                    {
                                        close = Some(index);
                                    }
                                });
                            });
                        }
                    });
            });
        if let Some(index) = close {
            self.close_tab(ctx, index);
        } else if let Some(index) = activate {
            self.activate(index);
            self.toolbar_focused = false;
            self.search = None;
        }
    }

    pub(super) fn workspace_header(&mut self, ctx: &Context, palette: &Palette) {
        egui::TopBottomPanel::top("workspace header")
            .frame(chrome_frame(palette, Margin::symmetric(16, 8)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.toolbar_actions(ui, palette);
                        ui.separator();
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let title = self.tabs[self.active].display_name();
                            let (icon, _) =
                                ui.allocate_exact_size(egui::vec2(31.0, 31.0), Sense::hover());
                            ui.painter().rect_filled(icon, 4, palette.accent_surface);
                            chrome::draw_icon(ui, icon.center(), Icon::Terminal, palette.accent);
                            ui.add(
                                egui::Label::new(RichText::new(&title).font(theme::emphasis(16.0)))
                                    .truncate(),
                            )
                            .on_hover_text(title);
                        });
                    });
                });
            });
    }

    pub(super) fn toolbar_actions(&mut self, ui: &mut Ui, palette: &Palette) {
        let compact = ui.ctx().content_rect().width() < 560.0;
        if chrome::control(ui, palette, 34.0, Icon::Output, "Tools", None).clicked() {
            self.labs.open = true;
        }
        if chrome::control(ui, palette, 34.0, Icon::Settings, "Settings", None).clicked() {
            self.action(ui.ctx(), Action::Settings);
        }
        if chrome::control(ui, palette, 34.0, Icon::Commands, "Command Palette", None)
            .on_hover_text(shortcut("⇧⌘P", "Ctrl+Shift+P"))
            .clicked()
        {
            self.action(ui.ctx(), Action::CommandPalette);
        }
        if chrome::control(
            ui,
            palette,
            if compact { 34.0 } else { 78.0 },
            Icon::Ai,
            "Ask AI",
            None,
        )
        .clicked()
        {
            self.action(ui.ctx(), Action::AskAi);
        }
        let find = chrome::control(
            ui,
            palette,
            if compact { 34.0 } else { 72.0 },
            Icon::Find,
            "Find",
            None,
        );
        if std::mem::take(&mut self.focus_toolbar) {
            find.request_focus();
        }
        if find.clicked() {
            self.action(ui.ctx(), Action::Search);
        }
        if ui.ctx().content_rect().width() >= SIDEBAR_MIN_WINDOW {
            let title = if self.sidebar_visible {
                "Hide Sidebar"
            } else {
                "Show Sidebar"
            };
            if chrome::control(ui, palette, 34.0, Icon::Sidebar, title, None).clicked() {
                self.action(ui.ctx(), Action::ToggleSidebar);
            }
        }
    }

    pub(super) fn toolbar(&mut self, ctx: &Context, palette: &Palette) {
        let mut activate = None;
        let mut close = None;
        let mut add = false;
        egui::TopBottomPanel::top("tab bar")
            .frame(chrome_frame(palette, Margin::symmetric(12, 5)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(32.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.toolbar_actions(ui, palette);
                        ui.separator();
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let visible_width = ui.available_width() - 36.0;
                            let tab_width = 105.0;
                            let overflow = self.tabs.len() as f32 * tab_width > visible_width;
                            let room = visible_width - if overflow { 58.0 } else { 0.0 };
                            let visible = ((room / tab_width) as usize).clamp(1, self.tabs.len());
                            let first = self.active.saturating_sub(visible - 1);
                            for (index, tab) in
                                self.tabs.iter().enumerate().skip(first).take(visible)
                            {
                                let active = index == self.active;
                                let title = tab.display_name();
                                let fill = if active {
                                    palette.accent_surface
                                } else {
                                    palette.chrome
                                };
                                ui.push_id(tab.id, |ui| {
                                    Frame::new()
                                        .fill(fill)
                                        .corner_radius(6)
                                        .inner_margin(Margin::symmetric(3, 0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 2.0;
                                                let label = RichText::new(&title);
                                                let label = if active {
                                                    label
                                                        .font(theme::emphasis(13.0))
                                                        .color(palette.accent)
                                                } else {
                                                    label
                                                };
                                                let response = ui
                                                    .add_sized(
                                                        [61.0, 30.0],
                                                        Button::new(label)
                                                            .truncate()
                                                            .frame_when_inactive(false),
                                                    )
                                                    .on_hover_text(&title);
                                                response.widget_info(|| {
                                                    egui::WidgetInfo::selected(
                                                        egui::WidgetType::SelectableLabel,
                                                        true,
                                                        active,
                                                        &title,
                                                    )
                                                });
                                                if response.clicked() {
                                                    activate = Some(index);
                                                }
                                                if icon_button(
                                                    ui,
                                                    "×",
                                                    &format!("Close {title}"),
                                                    true,
                                                )
                                                .clicked()
                                                {
                                                    close = Some(index);
                                                }
                                            });
                                        });
                                });
                            }
                            if overflow {
                                ui.menu_button("Tabs", |ui| {
                                    egui::ScrollArea::vertical()
                                        .max_height(220.0)
                                        .show(ui, |ui| {
                                            for (index, tab) in self.tabs.iter().enumerate() {
                                                let title = tab.display_name();
                                                if ui
                                                    .selectable_label(
                                                        index == self.active,
                                                        elide(&title, 42),
                                                    )
                                                    .on_hover_text(&title)
                                                    .clicked()
                                                {
                                                    activate = Some(index);
                                                    ui.close();
                                                }
                                            }
                                        });
                                })
                                .response
                                .on_hover_text("Switch to any open tab");
                            }
                            add = icon_button(ui, "+", "New Tab", true)
                                .on_hover_text(shortcut("New Tab · ⌘T", "New Tab · Ctrl+Shift+T"))
                                .clicked();
                        });
                    });
                });
            });
        if let Some(index) = close {
            self.close_tab(ctx, index);
        } else if let Some(index) = activate {
            self.activate(index);
            self.toolbar_focused = false;
            self.search = None;
        } else if add {
            self.new_tab(ctx);
        }
    }

    pub(super) fn settings(&mut self, ui: &mut Ui) {
        let width = (ui.ctx().content_rect().width() - 48.0).clamp(320.0, 510.0);
        ui.set_width(width);
        chrome::brand(ui);
        ui.add_space(10.0);
        ui.label(RichText::new("Settings").font(theme::emphasis(16.0)));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (page, title) in [
                (SettingsPage::Appearance, "Appearance"),
                (SettingsPage::Ai, "AI"),
                (SettingsPage::Shortcuts, "Shortcuts"),
            ] {
                ui.selectable_value(&mut self.settings_page, page, title);
            }
        });
        ui.separator();
        let panel_height = (ui.ctx().content_rect().height() - 160.0).max(80.0);
        egui::ScrollArea::vertical().max_height(panel_height).show(ui, |ui| {
            ui.add_space(4.0);
            match self.settings_page {
                SettingsPage::Appearance => {
                    setting_title(ui, "Theme", "Choose how the window and terminal look.");
                    ui.horizontal(|ui| {
                        for (mode, label) in [(ThemePreference::System, "System"), (ThemePreference::Light, "Light"), (ThemePreference::Dark, "Dark")] {
                            if ui.selectable_value(&mut self.preference, mode, label).changed() {
                                ui.ctx().set_theme(mode);
                            }
                        }
                    });
                    ui.add_space(12.0);
                    setting_title(ui, "Terminal text size", "Applies to every session.");
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut self.font_size, MIN_FONT_SIZE..=MAX_FONT_SIZE).integer().suffix(" pt"));
                        if ui.button("Reset").clicked() {
                            self.font_size = FONT_SIZE;
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Cursor");
                        egui::ComboBox::from_id_salt("cursor preference").selected_text(self.cursor_style.label()).show_ui(ui, |ui| {
                            for style in theme::CursorStyle::OPTIONS {
                                ui.selectable_value(&mut self.cursor_style, style, style.label());
                            }
                        });
                    });
                    ui.checkbox(&mut self.sidebar_visible, "Show session sidebar in wide windows");
                    ui.add_space(8.0);
                    if ui.add_enabled(self.tabs[self.active].session.is_some(), Button::new("Read Visible Output")).clicked() {
                        self.settings_open = false;
                        self.action(ui.ctx(), Action::ReadOutput);
                    }
                }
                SettingsPage::Ai => {
                    setting_title(ui, "Model", "A local runner or an API key from a provider.");
                    if ui.checkbox(&mut self.ai_preferences.enabled, "Enable Ask AI").changed() && !self.ai_preferences.enabled {
                        self.assistant.discard();
                    }
                    ui.add_space(9.0);
                    self.ai_preferences.provider_settings(ui, width);
                    ui.add_space(8.0);
                    ui.label("Attach terminal text manually and review it before sending. Nerd keeps questions and responses in memory for this run.");
                }
                SettingsPage::Shortcuts => shortcut_list(ui),
            }
        });
    }

    pub(super) fn shortcut_bar(&mut self, ctx: &Context, palette: &Palette) {
        egui::TopBottomPanel::bottom("shortcut bar")
            .exact_height(36.0)
            .frame(chrome_frame(palette, Margin::symmetric(10, 4)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let discover = Button::new(RichText::new("Discover Crawn.ai").size(12.0).color(palette.action_text))
                        .fill(palette.action)
                        .corner_radius(4.0)
                        .min_size(Vec2::new(0.0, 26.0));
                    if ui.add(discover).clicked() {
                        self.discover_open = true;
                    }
                    ui.add_space(4.0);
                    for (label, keys, action) in [
                        ("Ask AI", shortcut("Shift+Space", "Shift+Space"), Action::AskAi),
                        ("Find file", shortcut("⇧⌘Space", "Ctrl+Shift+Space"), Action::FindPath),
                        ("Commands", shortcut("⇧⌘P", "Ctrl+Shift+P"), Action::CommandPalette),
                        ("Settings", shortcut("⌘,", "Ctrl+,"), Action::Settings),
                    ] {
                        let button = egui::Button::new(
                            RichText::new(format!("{label}  {keys}")).size(12.0),
                        )
                        .min_size(egui::vec2(0.0, 26.0));
                        if ui.add(button).clicked() {
                            self.action(ctx, action);
                        }
                    }
                });
            });
    }

    pub(super) fn search_bar(&mut self, ctx: &Context, palette: &Palette) {
        let Some(search) = &mut self.search else {
            return;
        };
        let mut close = false;
        let mut seek = false;
        let mut direction = 0;
        egui::TopBottomPanel::top("find bar")
            .frame(chrome_frame(palette, Margin::symmetric(15, 6)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let field_width = (ui.available_width() - 232.0).clamp(100.0, 330.0);
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut search.query)
                            .id_salt(SEARCH_ID)
                            .hint_text("Find in scrollback")
                            .desired_width(field_width),
                    );
                    if std::mem::take(&mut search.focus) {
                        field.request_focus();
                    }
                    if field.changed() {
                        search.matches = self.tabs[self.active]
                            .session
                            .as_ref()
                            .map_or_else(Vec::new, |session| session.find(&search.query));
                        search.index = 0;
                        seek = true;
                    }
                    let count = search.matches.len();
                    let summary = if search.query.is_empty() {
                        String::new()
                    } else if count == 0 {
                        "No matches".to_owned()
                    } else {
                        format!("{} of {count}", search.index + 1)
                    };
                    ui.label(RichText::new(summary).color(palette.muted));
                    if ui
                        .add_enabled(count > 1, Button::new("Older"))
                        .on_hover_text("Older Match")
                        .clicked()
                    {
                        direction = 1;
                    }
                    if ui
                        .add_enabled(count > 1, Button::new("Newer"))
                        .on_hover_text("Newer Match")
                        .clicked()
                    {
                        direction = -1;
                    }
                    if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        direction = if ui.input(|i| i.modifiers.shift) {
                            -1
                        } else {
                            1
                        };
                        field.request_focus();
                    }
                    close = icon_button(ui, "×", "Close Find", true).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape));
                });
            });
        if !search.matches.is_empty() && direction != 0 {
            search.index =
                (search.index as i32 + direction).rem_euclid(search.matches.len() as i32) as usize;
            seek = true;
        }
        if seek {
            let tab = &mut self.tabs[self.active];
            tab.selection = None;
            if let (Some(session), Some(found)) = (&tab.session, search.matches.get(search.index)) {
                session.seek(found.scrollback_offset);
            }
        }
        if close {
            self.search = None;
        }
    }
}
