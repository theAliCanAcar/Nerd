use alacritty_terminal::term::TermMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Enter,
    Tab,
    Backspace,
    Escape,
    Function(u8),
    Character(char),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

impl Modifiers {
    fn key_code(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.ctrl)
    }

    fn mouse_bits(self) -> u8 {
        4 * u8::from(self.shift) + 8 * u8::from(self.alt) + 16 * u8::from(self.ctrl)
    }
}

pub(crate) fn key(key: Key, modifiers: Modifiers, modes: TermMode) -> Vec<u8> {
    let modifier = modifiers.key_code();
    let cursor_key = |suffix| {
        if modifier != 1 {
            format!("\x1b[1;{modifier}{suffix}").into_bytes()
        } else if modes.contains(TermMode::APP_CURSOR) {
            vec![b'\x1b', b'O', suffix as u8]
        } else {
            vec![b'\x1b', b'[', suffix as u8]
        }
    };
    let tilde_key = |number| {
        if modifier == 1 {
            format!("\x1b[{number}~").into_bytes()
        } else {
            format!("\x1b[{number};{modifier}~").into_bytes()
        }
    };
    match key {
        Key::Up => cursor_key('A'),
        Key::Down => cursor_key('B'),
        Key::Right => cursor_key('C'),
        Key::Left => cursor_key('D'),
        Key::Home => cursor_key('H'),
        Key::End => cursor_key('F'),
        Key::Insert => tilde_key(2),
        Key::Delete => tilde_key(3),
        Key::PageUp => tilde_key(5),
        Key::PageDown => tilde_key(6),
        Key::Function(n @ 1..=4) => {
            let suffix = (b'P' + n - 1) as char;
            if modifier == 1 {
                vec![b'\x1b', b'O', suffix as u8]
            } else {
                format!("\x1b[1;{modifier}{suffix}").into_bytes()
            }
        }
        Key::Function(n @ 5..=12) => {
            tilde_key([15, 17, 18, 19, 20, 21, 23, 24][usize::from(n - 5)])
        }
        Key::Function(_) => Vec::new(),
        Key::Tab if modifiers.shift => b"\x1b[Z".to_vec(),
        Key::Enter => alt_prefix(b"\r", modifiers.alt),
        Key::Tab => alt_prefix(b"\t", modifiers.alt),
        Key::Escape => alt_prefix(b"\x1b", modifiers.alt),
        Key::Backspace => alt_prefix(
            if modifiers.ctrl { b"\x08" } else { b"\x7f" },
            modifiers.alt,
        ),
        Key::Character(c) => {
            if modifiers.ctrl {
                return control_code(c)
                    .map_or_else(Vec::new, |control| alt_prefix(&[control], modifiers.alt));
            }
            alt_prefix(c.encode_utf8(&mut [0; 4]).as_bytes(), modifiers.alt)
        }
    }
}

fn control_code(c: char) -> Option<u8> {
    match c.to_ascii_uppercase() {
        'A'..='Z' => Some(c.to_ascii_uppercase() as u8 & 31),
        ' ' | '@' | '2' => Some(0),
        '[' | '3' => Some(27),
        '\\' | '4' => Some(28),
        ']' | '5' => Some(29),
        '^' | '6' => Some(30),
        '_' | '/' | '7' => Some(31),
        '?' | '8' => Some(127),
        _ => None,
    }
}

fn alt_prefix(bytes: &[u8], alt: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + usize::from(alt));
    if alt {
        out.push(b'\x1b');
    }
    out.extend_from_slice(bytes);
    out
}

pub(crate) fn paste(text: &str, modes: TermMode) -> Vec<u8> {
    let bracketed = modes.contains(TermMode::BRACKETED_PASTE);
    let mut out = Vec::with_capacity(text.len());
    if bracketed {
        out.extend_from_slice(b"\x1b[200~");
    }
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        // Normalize CRLF before applying the shell mode.
        if c == '\r' && chars.peek() == Some(&'\n') {
            continue;
        }
        match c {
            '\r' | '\n' => out.push(if bracketed { b'\n' } else { b'\r' }),
            '\t' => out.push(b'\t'),
            c if !c.is_control() => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            _ => {}
        }
    }
    if bracketed {
        out.extend_from_slice(b"\x1b[201~");
    }
    out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Left = 0,
    Middle = 1,
    Right = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseEvent {
    Press(MouseButton),
    Release(MouseButton),
    Move(Option<MouseButton>),
    WheelUp,
    WheelDown,
}

pub(crate) fn mouse(
    event: MouseEvent,
    row: usize,
    col: usize,
    modifiers: Modifiers,
    modes: TermMode,
) -> Option<Vec<u8>> {
    if !modes.intersects(TermMode::MOUSE_MODE) {
        if modes.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let arrow = match event {
                MouseEvent::WheelUp => Key::Up,
                MouseEvent::WheelDown => Key::Down,
                _ => return None,
            };
            return Some(key(arrow, modifiers, modes));
        }
        return None;
    }

    let sgr = modes.contains(TermMode::SGR_MOUSE);
    let (button, released) = match event {
        MouseEvent::Press(button) => (button as u8, false),
        MouseEvent::Release(button) if sgr => (button as u8, true),
        MouseEvent::Release(_) => (3, true),
        MouseEvent::Move(button) => {
            let track_hover = modes.contains(TermMode::MOUSE_MOTION);
            let track_drag = modes.contains(TermMode::MOUSE_DRAG) && button.is_some();
            if !(track_hover || track_drag) {
                return None;
            }
            (button.map_or(3, |button| button as u8) + 32, false)
        }
        MouseEvent::WheelUp => (64, false),
        MouseEvent::WheelDown => (65, false),
    };
    let button = button + modifiers.mouse_bits();
    if sgr {
        let suffix = if released { 'm' } else { 'M' };
        return Some(format!("\x1b[<{button};{};{}{suffix}", col + 1, row + 1).into_bytes());
    }

    let utf8 = modes.contains(TermMode::UTF8_MOUSE);
    let max = if utf8 { 2015 } else { 223 };
    if row >= max || col >= max {
        // Do not send a coordinate that would overflow the selected protocol.
        return Some(Vec::new());
    }
    let mut bytes = vec![b'\x1b', b'[', b'M', button + 32];
    for coord in [col, row] {
        if utf8 {
            let scalar = char::from_u32(coord as u32 + 33).unwrap();
            bytes.extend_from_slice(scalar.encode_utf8(&mut [0; 4]).as_bytes());
        } else {
            bytes.push((coord + 33) as u8);
        }
    }
    Some(bytes)
}
