# Implementation status

This note describes the current build. See [compatibility checks](compatibility.md) for the manual pass.

| Area | In this build | Still ahead |
| --- | --- | --- |
| Terminal | Local PTY, GPU window, tabs, splits, scrollback search, clipboard, paste confirmation, command palette, macOS menu, light and dark appearance | Multiple windows, layout restore, shell profiles, configurable keybindings, fuller protocol and accessibility checks |
| Ask AI | Off until you turn it on. A local model, or a key you paste. The text to send is shown before it leaves the machine. A command is not run until you approve it. A pasted key is stored in the system keychain. | Streaming and cancellation |
| Checks | The Rust workspace builds and tests on macOS, Windows, and Ubuntu in CI. CI does not open the desktop window. | Interactive Linux and Windows desktop qualification |
