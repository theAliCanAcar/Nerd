use super::*;

#[cfg(unix)]
mod pty {
    use std::time::{Duration, Instant};

    use super::*;

    fn launch(cols: u16, rows: u16, script: &str) -> Session {
        let options = tty::Options {
            shell: Some(tty::Shell::new(
                "/bin/sh".into(),
                vec!["-c".into(), script.into()],
            )),
            ..Default::default()
        };
        Session::launch(cols, rows, options, || {}).unwrap()
    }

    fn wait_for(session: &Session, condition: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = session.snapshot();
            if condition(&snapshot) {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "terminal did not reach expected state: {:?}",
                snapshot
                    .cells
                    .iter()
                    .map(|cell| &cell.text)
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<_>>()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn shell_round_trips_input_resize_and_exit() {
        let shell = launch(
            40,
            6,
            "stty -echo; printf 'Ready\n'; IFS= read -r line; stty size; printf '%s\n' \"$line\"; exit 7",
        );
        wait_for(&shell, |view| {
            view.cells.first().is_some_and(|cell| cell.text == "R")
        });
        shell.resize(72, 12).unwrap();
        shell.write(b"hello from the PTY\r").unwrap();
        let view = wait_for(&shell, |view| view.exited);
        assert_eq!(view.exit_code, Some(7));
        assert_eq!((view.cols, view.rows), (72, 12));
        assert_eq!(view.cells.len(), 72 * 12);
        let text = shell.copy((0, 0), (11, 71));
        assert!(text.contains("12 72"), "{text:?}");
        assert!(text.contains("hello from the PTY"), "{text:?}");
    }

    #[test]
    fn output_preserves_styles_wide_text_and_scrollback() {
        let shell = launch(
            32,
            3,
            r"printf 'searchable 界!\nline 2\nline 3\nline 4\n\033[38;2;10;20;30m界e\314\201\033[0m'",
        );
        let view = wait_for(&shell, |view| view.exited);
        assert!(view.history_size > 0);
        let index = view.cells.iter().position(|cell| cell.wide).unwrap();
        assert_eq!(view.cells[index].text, "界");
        assert_eq!(view.cells[index].fg, Color::Rgb([10, 20, 30]));
        assert!(view.cells[index + 1].spacer);
        assert_eq!(view.cells[index + 2].text, "e\u{301}");

        let found = shell.find("searchable 界").into_iter().next().unwrap();
        assert_eq!(found.len, 13);
        shell.seek(found.scrollback_offset);
        assert_eq!(
            shell.copy((found.row, 0), (found.row, 13)),
            "searchable 界!"
        );
    }
}

#[test]
fn application_cursor_control_keys_and_paste_framing() {
    assert_eq!(
        input::key(Key::Left, Modifiers::default(), TermMode::APP_CURSOR),
        b"\x1bOD"
    );
    assert_eq!(
        input::key(
            Key::Right,
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
            TermMode::APP_CURSOR
        ),
        b"\x1b[1;5C"
    );
    assert_eq!(
        input::key(
            Key::Character('c'),
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
            TermMode::empty()
        ),
        b"\x03"
    );
    assert_eq!(
        input::paste("one\r\ntwo\x1b[201~\x03", TermMode::BRACKETED_PASTE),
        b"\x1b[200~one\ntwo[201~\x1b[201~"
    );
    assert_eq!(
        input::paste("one\r\ntwo\n", TermMode::empty()),
        b"one\rtwo\r"
    );
}

#[test]
fn mouse_uses_sgr_release_coordinates_when_requested() {
    assert_eq!(
        input::mouse(
            MouseEvent::Release(MouseButton::Left),
            4,
            9,
            Modifiers::default(),
            TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE
        ),
        Some(b"\x1b[<0;10;5m".to_vec())
    );
    assert_eq!(
        input::mouse(
            MouseEvent::Move(None),
            4,
            9,
            Modifiers::default(),
            TermMode::MOUSE_DRAG
        ),
        None
    );
}
