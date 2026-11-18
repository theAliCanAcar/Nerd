use nerd_terminal::{Key, Modifiers};
use eframe::egui::{self, Key as EguiKey};

#[derive(Clone, Copy)]
pub enum Action {
    NewTab,
    CloseTab,
    SplitRight,
    SplitDown,
    Search,
    ToggleSidebar,
    Settings,
    AskAi,
    FindPath,
    CommandPalette,
    RenameSession,
    Shortcuts,
    ReadOutput,
    FocusToolbar,
    Copy,
    Paste,
    PreviousTab,
    NextTab,
    ZoomIn,
    ZoomOut,
    ResetZoom,
}

pub const MAC: bool = cfg!(target_os = "macos");

pub fn app_shortcut(key: EguiKey, modifiers: egui::Modifiers) -> Option<Action> {
    if key == EguiKey::Space && modifiers.shift && !modifiers.ctrl && !modifiers.alt {
        if modifiers.command {
            return Some(Action::FindPath);
        }
        return Some(Action::AskAi);
    }
    if modifiers.command
        && ((MAC && modifiers.alt && !modifiers.shift && key == EguiKey::S)
            || (!MAC && !modifiers.alt && modifiers.shift && key == EguiKey::B))
    {
        return Some(Action::ToggleSidebar);
    }
    if key == EguiKey::D && modifiers.command {
        if MAC && !modifiers.alt && modifiers.shift {
            return Some(Action::SplitDown);
        }
        if MAC && !modifiers.alt && !modifiers.shift {
            return Some(Action::SplitRight);
        }
        if !MAC && modifiers.shift && modifiers.alt {
            return Some(Action::SplitDown);
        }
        if !MAC && modifiers.shift && !modifiers.alt {
            return Some(Action::SplitRight);
        }
    }
    if !modifiers.command || modifiers.alt {
        return None;
    }
    match key {
        EguiKey::T if MAC || modifiers.shift => Some(Action::NewTab),
        EguiKey::W if MAC || modifiers.shift => Some(Action::CloseTab),
        EguiKey::F if MAC || modifiers.shift => Some(Action::Search),
        EguiKey::I if modifiers.shift => Some(Action::AskAi),
        EguiKey::P if modifiers.shift => Some(Action::CommandPalette),
        EguiKey::Comma if MAC => Some(Action::Settings),
        EguiKey::F6 if modifiers.shift => Some(Action::FocusToolbar),
        EguiKey::C if !MAC && modifiers.shift => Some(Action::Copy),
        EguiKey::V if !MAC && modifiers.shift => Some(Action::Paste),
        EguiKey::OpenBracket | EguiKey::OpenCurlyBracket if MAC && modifiers.shift => {
            Some(Action::PreviousTab)
        }
        EguiKey::CloseBracket | EguiKey::CloseCurlyBracket if MAC && modifiers.shift => {
            Some(Action::NextTab)
        }
        EguiKey::PageUp if !MAC => Some(Action::PreviousTab),
        EguiKey::PageDown if !MAC => Some(Action::NextTab),
        EguiKey::Plus | EguiKey::Equals => Some(Action::ZoomIn),
        EguiKey::Minus => Some(Action::ZoomOut),
        EguiKey::Num0 => Some(Action::ResetZoom),
        _ => None,
    }
}

pub fn terminal_key(key: EguiKey, m: egui::Modifiers) -> Option<(Key, Modifiers)> {
    if m.mac_cmd {
        return None;
    }
    let mods = Modifiers {
        ctrl: m.ctrl,
        alt: m.alt,
        shift: m.shift,
    };
    let key = match key {
        EguiKey::ArrowUp => Key::Up,
        EguiKey::ArrowDown => Key::Down,
        EguiKey::ArrowLeft => Key::Left,
        EguiKey::ArrowRight => Key::Right,
        EguiKey::Home => Key::Home,
        EguiKey::End => Key::End,
        EguiKey::PageUp => Key::PageUp,
        EguiKey::PageDown => Key::PageDown,
        EguiKey::Insert => Key::Insert,
        EguiKey::Delete => Key::Delete,
        EguiKey::Enter => Key::Enter,
        EguiKey::Tab => Key::Tab,
        EguiKey::Backspace => Key::Backspace,
        EguiKey::Escape => Key::Escape,
        EguiKey::F1 => Key::Function(1),
        EguiKey::F2 => Key::Function(2),
        EguiKey::F3 => Key::Function(3),
        EguiKey::F4 => Key::Function(4),
        EguiKey::F5 => Key::Function(5),
        EguiKey::F6 => Key::Function(6),
        EguiKey::F7 => Key::Function(7),
        EguiKey::F8 => Key::Function(8),
        EguiKey::F9 => Key::Function(9),
        EguiKey::F10 => Key::Function(10),
        EguiKey::F11 => Key::Function(11),
        EguiKey::F12 => Key::Function(12),
        EguiKey::Space if m.ctrl => Key::Character(' '),
        _ if m.ctrl && key.symbol_or_name().len() == 1 => {
            Key::Character(key.symbol_or_name().chars().next()?.to_ascii_lowercase())
        }
        _ => return None,
    };
    Some((key, mods))
}
