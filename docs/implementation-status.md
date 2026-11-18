# Project implementation status

This review maps the current development build to the planned phases.

| Phase | Available in this build | Remaining |
| --- | --- | --- |
| 0 · Feasibility and design | Rust/wgpu desktop build, real PTY, Nerd styling, original development icon, MIT license | Approved brand assets, measured performance baseline, interactive Linux and Windows qualification |
| 1 · Terminal alpha | Tabbed shells, session renaming, scrollback search, clipboard, paste confirmation, keyboard command palette, macOS menu, accessible visible-output view, persisted appearance and cursor settings, compact layout | Split panes, multiple windows, layout restore, shell profiles and hooks, configurable keybindings, richer font and protocol support, full accessibility and terminal compatibility validation |
| 2 · AI and Guardian | Opt-in Ollama, installed-model picker, question and optional terminal context preview, bounded requests, text responses that can be copied | Cloud and compatible provider adapters, BYOK secret storage, capability discovery, streaming and cancellation, Guardian and execution policies, provider and security matrix |
| 3 · Agent Arena | | Agent adapters, isolation, worktrees, comparisons, test results, and approval flow |
| 4 · Time Machine and Project Brain | | Opt-in structured history, cited project memory, evidence export, and recovery previews |
| 5 · Live Doctor and polish | | Service health, correlated logs, diagnostics, platform packaging, and internationalization |
| 6 · Stable release | | Cross-platform runtime, accessibility and security reviews, release distribution, and upgrade verification |

See [compatibility coverage](compatibility.md) for the checks that back the terminal claims.
