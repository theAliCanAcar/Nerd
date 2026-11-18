# Contributing to Nerd

Nerd is in early development. The [terminal verification guide](docs/compatibility.md) covers the current desktop build. The [implementation status](docs/implementation-status.md) lists what is in this repository.

## Set up the workspace

Install [Rust via rustup](https://rustup.rs/). Run the commands from the workspace root so rustup selects the toolchain in `rust-toolchain.toml`.

- **macOS** — Install Xcode Command Line Tools with `xcode-select --install` if the compiler and SDK are missing.
- **Windows** — Use the Rust MSVC toolchain. Install Visual Studio Build Tools with the Desktop development with C++ workload and a Windows SDK.
- **Linux** — Use a Wayland or X11 desktop and a working graphics driver. On Ubuntu or Debian, install the build dependencies below.

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config libwayland-dev libx11-dev libxkbcommon-dev
```

Build and launch from the workspace:

```sh
cargo run --locked -p nerd
```

## Verify a change

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
cargo test --locked --workspace
```

For changes to input, rendering, clipboard, process handling, or desktop controls, also run the relevant [interactive checks](docs/compatibility.md). Include the operating system, desktop environment, shell, and reproduction steps when reporting a failure. Use synthetic output and remove credentials, local paths, and private repository details from logs or screenshots.

Keep fixes focused and include a regression test when an existing test would miss the bug. Update `Cargo.lock` with dependency changes. Cite the upstream license and explain the need for any new runtime dependency.
