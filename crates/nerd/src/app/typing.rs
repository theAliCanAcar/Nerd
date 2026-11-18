use super::*;

pub(super) struct TerminalInput<'a> {
    pub(super) session: &'a dyn super::host::Shell,
    pub(super) selection: &'a mut Option<Selection>,
    pub(super) composition: &'a mut String,
    pub(super) mouse_button: &'a mut Option<MouseButton>,
    pub(super) mouse_cell: &'a mut Option<CellPosition>,
    pub(super) wheel: &'a mut f32,
    pub(super) selecting: &'a mut bool,
    pub(super) writable: bool,
    pub(super) pending_paste: &'a mut Option<String>,
    pub(super) focus_cancel: &'a mut bool,
    pub(super) error: &'a mut Option<String>,
    pub(super) used: &'a mut bool,
    pub(super) draft: &'a mut String,
    pub(super) phrase: &'a mut Option<String>,
    pub(super) can_convert: bool,
}

impl TerminalInput<'_> {
    pub(super) fn process(
        &mut self,
        ctx: &Context,
        events: &[Event],
        grid: &Geometry,
        pointer: Option<egui::Pos2>,
        snapshot: &nerd_terminal::Snapshot,
    ) -> bool {
        let modifiers = ctx.input(|i| i.modifiers);
        let mut changed = false;
        for event in events {
            let result = match event {
                Event::Copy if MAC || modifiers.shift || !modifiers.ctrl => {
                    if let Some(selection) = *self.selection {
                        let text = self.session.copy(selection.anchor, selection.end);
                        if !text.is_empty() {
                            ctx.copy_text(text);
                        }
                    }
                    continue;
                }
                Event::Copy if self.writable => self.session.write(&[0x03]),
                Event::Cut if self.writable && !MAC && modifiers.ctrl && !modifiers.shift => {
                    self.session.write(&[0x18])
                }
                Event::Paste(_) if self.writable && !MAC && modifiers.ctrl && !modifiers.shift => {
                    self.session.write(&[0x16])
                }
                Event::Paste(text) if self.writable => {
                    if text.contains(['\r', '\n']) {
                        *self.pending_paste = Some(text.clone());
                        *self.focus_cancel = true;
                        break;
                    }
                    self.session.paste(text)
                }
                Event::Text(text) if self.writable => {
                    if !MAC && modifiers.alt && !modifiers.ctrl && text.is_ascii() {
                        let mut bytes = vec![0x1b];
                        bytes.extend_from_slice(text.as_bytes());
                        self.session.write(&bytes)
                    } else {
                        self.draft.push_str(text);
                        self.session.write(text.as_bytes())
                    }
                }
                Event::Ime(ImeEvent::Commit(text)) if self.writable => {
                    self.composition.clear();
                    self.session.write(text.as_bytes())
                }
                Event::Ime(ImeEvent::Preedit(text)) => {
                    self.composition.clone_from(text);
                    continue;
                }
                Event::Ime(ImeEvent::Enabled | ImeEvent::Disabled) => {
                    self.composition.clear();
                    continue;
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let select_all = *key == Key::A
                        && modifiers.command
                        && (MAC || modifiers.shift)
                        && !(MAC && modifiers.shift);
                    if select_all {
                        *self.selection = Some(grid::screen_selection(snapshot));
                        changed = true;
                        continue;
                    }
                    if modifiers.shift
                        && !modifiers.alt
                        && !modifiers.command
                        && matches!(key, Key::PageUp | Key::PageDown)
                    {
                        let direction = if *key == Key::PageUp { 1 } else { -1 };
                        self.session.scroll(direction * grid.rows as i32);
                        *self.selection = None;
                        changed = true;
                        continue;
                    }
                    if !self.writable {
                        continue;
                    }
                    let Some((key, mods)) = input::terminal_key(*key, *modifiers) else {
                        continue;
                    };
                    if key == TermKey::Enter
                        && !mods.ctrl
                        && !mods.alt
                        && self.can_convert
                        && self.phrase.is_none()
                        && ai::looks_like_prose(self.draft)
                    {
                        let line = std::mem::take(self.draft);
                        let erasures = vec![0x7f; line.chars().count()];
                        let _ = self.session.write(&erasures);
                        *self.phrase = Some(line);
                        *self.used = true;
                        continue;
                    }
                    if key == TermKey::Backspace {
                        self.draft.pop();
                    } else if key == TermKey::Enter {
                        self.draft.clear();
                    }
                    self.session.key(key, mods)
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => {
                    let Some(button) = convert_button(*button) else {
                        continue;
                    };
                    let cell = grid.cell(*pos);
                    let inside = grid.rect.contains(*pos);
                    if !inside && (*pressed || self.mouse_button.is_none()) {
                        if !pressed {
                            *self.selecting = false;
                        }
                        continue;
                    }
                    let event = if *pressed {
                        MouseEvent::Press(button)
                    } else {
                        MouseEvent::Release(button)
                    };
                    let reported = if modifiers.shift {
                        false
                    } else {
                        self.mouse(event, cell, *modifiers)
                    };
                    if *pressed && reported {
                        *self.mouse_button = Some(button);
                        *self.mouse_cell = Some(cell);
                        *self.selection = None;
                        *self.selecting = false;
                    } else if !pressed {
                        let click = *self.selecting
                            && self.selection.is_some_and(|selection| selection.anchor == selection.end)
                            && matches!(button, MouseButton::Left);
                        *self.mouse_button = None;
                        *self.selecting = false;
                        if click {
                            *self.selection = None;
                            self.move_caret(cell);
                        }
                    } else if matches!(button, MouseButton::Left) {
                        let triple = ctx.input(|i| i.pointer.button_triple_clicked(PointerButton::Primary));
                        let double = ctx.input(|i| i.pointer.button_double_clicked(PointerButton::Primary));
                        *self.selection = Some(if triple {
                            grid::line_selection(snapshot, cell.0)
                        } else if double {
                            grid::word_selection(snapshot, cell)
                        } else {
                            Selection { anchor: cell, end: cell }
                        });
                        *self.selecting = !triple && !double;
                    }
                    continue;
                }
                Event::PointerMoved(pos) => {
                    let cell = grid.cell(*pos);
                    if *self.selecting {
                        if let Some(selection) = self.selection {
                            selection.end = cell;
                        }
                    } else if grid.rect.contains(*pos)
                        && *self.mouse_cell != Some(cell)
                        && !modifiers.shift
                    {
                        self.mouse(MouseEvent::Move(*self.mouse_button), cell, modifiers);
                        *self.mouse_cell = Some(cell);
                    }
                    continue;
                }
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers: mods,
                } => {
                    let Some(pos) = pointer.filter(|pos| grid.rect.contains(*pos)) else {
                        continue;
                    };
                    let scale = match unit {
                        MouseWheelUnit::Point => 1.0 / grid.height,
                        MouseWheelUnit::Line => 3.0,
                        MouseWheelUnit::Page => grid.rows as f32,
                    };
                    *self.wheel += delta.y * scale;
                    let lines = (*self.wheel as i32).clamp(-32, 32);
                    *self.wheel -= lines as f32;
                    if lines != 0 {
                        let mouse = if lines > 0 {
                            MouseEvent::WheelUp
                        } else {
                            MouseEvent::WheelDown
                        };
                        let cell = grid.cell(pos);
                        if !mods.shift && self.mouse(mouse, cell, *mods) {
                            for _ in 1..lines.abs() {
                                self.mouse(mouse, cell, *mods);
                            }
                        } else {
                            self.session.scroll(lines);
                        }
                        *self.selection = None;
                        changed = true;
                    }
                    continue;
                }
                _ => continue,
            };
            *self.selection = None;
            *self.selecting = false;
            *self.used = true;
            self.session.scroll_to_bottom();
            if let Err(error) = result {
                *self.error = Some(format!("Could not write to the shell: {error:#}"));
            }
            changed = true;
        }
        changed
    }

    fn move_caret(&mut self, cell: CellPosition) {
        if !self.writable {
            return;
        }
        let snapshot = self.session.snapshot();
        if snapshot.scrollback_offset > 0 {
            self.session.scroll_to_bottom();
            return;
        }
        let Some((row, col)) = snapshot.cursor else {
            return;
        };
        if cell.0 != row || cell.1 == col {
            return;
        }
        let key = if cell.1 < col {
            TermKey::Left
        } else {
            TermKey::Right
        };
        let steps = cell.1.abs_diff(col).min(256);
        for _ in 0..steps {
            if self
                .session
                .key(
                    key,
                    Modifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                )
                .is_err()
            {
                break;
            }
        }
        *self.used = true;
    }

    fn mouse(
        &mut self,
        event: MouseEvent,
        (row, col): CellPosition,
        modifiers: egui::Modifiers,
    ) -> bool {
        if !self.writable {
            return false;
        }
        self.session
            .mouse(
                event,
                row,
                col,
                Modifiers {
                    shift: modifiers.shift,
                    ctrl: modifiers.ctrl,
                    alt: modifiers.alt,
                },
            )
            .unwrap_or_else(|error| {
                *self.error = Some(format!("Could not send shell mouse input: {error:#}"));
                false
            })
    }
}

fn convert_button(button: PointerButton) -> Option<MouseButton> {
    match button {
        PointerButton::Primary => Some(MouseButton::Left),
        PointerButton::Secondary => Some(MouseButton::Right),
        PointerButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}
