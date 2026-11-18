# Terminal verification

Run these checks on a desktop before calling a build compatible with a platform or shell. The [CI workflow](../.github/workflows/ci.yml) is configured to build and test the Rust workspace on macOS, Windows, and Ubuntu. Desktop launch, graphics, input, and integration checks require an interactive session.

## Initial development build

The Apple Silicon build launched from the local app bundle on macOS 26.4.1. The interactive pass covered native menu actions, switching to a hidden session with ten tabs open, restoring terminal focus from Find, running a shell command with ANSI color, cancelling a multiline paste with Return, interrupting a process with Ctrl+C, and keeping text size across a relaunch. The accessibility tree exposed visible output in the Read Visible Output view. The workspace also passed cross-target Rust checks for 64-bit Linux and Windows. Desktop runtime verification for those platforms is pending.

Search is case-sensitive and matches within a single terminal row. Scrollback stays in memory. The app stores theme, font size, and sidebar visibility locally. Advanced graphics and keyboard protocols, font fallback coverage, screen-reader access to the cell grid, and foreground-job detection when closing tabs need further implementation and testing.

The macOS UI pass also covered opening the command palette by shortcut, renaming a live session, finding an installed Ollama model, and receiving an answer from `llama3.2:latest` for a synthetic question without a terminal attachment.

| Environment | Desktop checks |
| --- | --- |
| macOS | Apple Silicon and Intel where available, default login shell, 1× and Retina displays |
| Windows | Windows 11, ConPTY with the default shell, clipboard and display scaling |
| Linux | Wayland and X11 tested separately, Bash and an additional installed shell, clipboard on each display server |

## Interactive smoke test

1. **Launch and offline use.** Build with `cargo build --locked -p nerd`. Disconnect networking and run `./target/debug/nerd` on macOS or Linux, or `.\target\debug\nerd.exe` on Windows. Confirm you get a local prompt and can run a basic command without entering credentials. A shell startup file may run its own network commands.
2. **Input and process lifecycle.** Type, edit, and submit a line. Check backspace, arrows, Home/End, command history, and Tab completion. Run `sleep 15` in a Unix shell or `ping -n 15 127.0.0.1` in CMD, then press Ctrl+C and check that the prompt returns. Exit a tab's shell and confirm the UI reports the session ended or closes the tab. Launch a fresh session.
3. **Resize and text.** Drag the window smaller and larger at the prompt and in a full-screen program. On Unix, compare `stty size` before and after resizing. Check contrasting ANSI colors, line wrapping, cursor placement, and selection across text such as `Café é 中文 🙂`. Record missing glyphs and cursor misalignment.
4. **Scrollback and clipboard.** Produce output taller than the viewport and scroll back. Select a short passage, copy it, and paste it into a disposable text editor. Compare the copied text with the terminal passage. For paste testing in a Unix shell, run `cat` first so pasted sample text becomes input to `cat`. Include multiline text and record any confirmation or bracketed paste behavior.
5. **Tabs and appearance.** Open two tabs, type distinct commands in each, and switch between them. Close one tab and confirm the other remains usable. Change the theme and text size, then check cursor alignment, tab selection and visual contrast. Verify typing still works after dismissing menus.
6. **Full-screen programs.** Where installed, open a disposable file in Vim or Neovim, use arrow keys, resize the window, and quit. Repeat with a pager such as `less`. Confirm the previous prompt and scrollback are restored when the program exits. Check `tmux`, `fzf`, mouse reporting, modified keys, IME composition, and screen-reader output separately.
7. **Session sidebar.** Open a shell from New session, then toggle the sidebar while that shell is active. Verify that the same shell and its output are visible in the compact tab layout. Narrow the window below 680 logical pixels and widen it again to check the automatic layout switch.

## Recording results

For each test, record the commit, OS and version, shell and version, display server where applicable, graphics adapter, display scale, and pass or fail. For failures, include the smallest command or keystroke sequence that reproduces the issue.

Platform coverage still needs interactive passes across the desktop matrix. Protocol conformance, including graphics sequences, keyboard extensions, OSC behavior, grapheme widths, and full-screen program handling, needs its own validation. Accessibility, GPU recovery, long-output performance, and distribution packaging require separate testing.
