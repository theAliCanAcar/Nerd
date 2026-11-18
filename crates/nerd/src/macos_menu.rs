use eframe::egui::{Context, Id, ViewportCommand};
use muda::{
    AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem as NativeItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};

use crate::input::Action;

#[derive(Clone, Copy)]
enum Command {
    App(Action),
    Cut,
    Copy,
    Paste,
}

pub fn disable_automatic_window_tabs() {
    // Nerd manages shell tabs inside one window. Suppress AppKit's unrelated window tab commands.
    let main_thread = objc2::MainThreadMarker::new().expect("the app starts on its main thread");
    objc2_app_kit::NSWindow::setAllowsAutomaticWindowTabbing(false, main_thread);
}

pub fn install(ctx: &Context) -> muda::Result<Menu> {
    let app = Submenu::with_items(
        "Nerd",
        true,
        &[
            &NativeItem::about(
                Some("About Nerd"),
                Some(AboutMetadata {
                    name: Some("Nerd".into()),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                    credits: Some("Built by Ali Can Acar. Crawn.ai is another product he created.".into()),
                    ..Default::default()
                }),
            ),
            &NativeItem::separator(),
            &item("settings", "Settings…", Code::Comma),
            &NativeItem::separator(),
            &NativeItem::services(None),
            &NativeItem::separator(),
            &NativeItem::hide(Some("Hide Nerd")),
            &NativeItem::hide_others(None),
            &NativeItem::show_all(None),
            &NativeItem::separator(),
            &NativeItem::quit(Some("Quit Nerd")),
        ],
    )?;
    let file = Submenu::with_items(
        "File",
        true,
        &[
            &item("new-tab", "New Tab", Code::KeyT),
            &MenuItem::with_id(
                "split-right",
                "Split Right",
                true,
                Some(Accelerator::new(Modifiers::META, Code::KeyD)),
            ),
            &MenuItem::with_id(
                "split-down",
                "Split Down",
                true,
                Some(Accelerator::new(Modifiers::META | Modifiers::SHIFT, Code::KeyD)),
            ),
            &MenuItem::with_id("rename-session", "Rename Session…", true, None),
            &item("close-tab", "Close Tab", Code::KeyW),
        ],
    )?;
    let edit = Submenu::with_items(
        "Edit",
        true,
        &[
            &item("cut", "Cut", Code::KeyX),
            &item("copy", "Copy", Code::KeyC),
            &item("paste", "Paste", Code::KeyV),
            &NativeItem::separator(),
            &item("find", "Find…", Code::KeyF),
        ],
    )?;
    let view = Submenu::with_items(
        "View",
        true,
        &[
            &MenuItem::with_id(
                "commands",
                "Command Palette…",
                true,
                Some(Accelerator::new(
                    Modifiers::META | Modifiers::SHIFT,
                    Code::KeyP,
                )),
            ),
            &MenuItem::with_id(
                "ask-ai",
                "Ask AI…",
                true,
                Some(Accelerator::new(
                    Modifiers::META | Modifiers::SHIFT,
                    Code::KeyI,
                )),
            ),
            &NativeItem::separator(),
            &MenuItem::with_id(
                "sidebar",
                "Toggle Sidebar",
                true,
                Some(Accelerator::new(
                    Modifiers::META | Modifiers::ALT,
                    Code::KeyS,
                )),
            ),
            &NativeItem::separator(),
            &item("increase-size", "Increase Text Size", Code::Equal),
            &item("decrease-size", "Decrease Text Size", Code::Minus),
            &item("actual-size", "Actual Size", Code::Digit0),
            &NativeItem::separator(),
            &MenuItem::with_id("read-output", "Read Visible Output…", true, None),
            &NativeItem::separator(),
            &NativeItem::fullscreen(None),
        ],
    )?;
    let shift_command = Modifiers::META | Modifiers::SHIFT;
    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &NativeItem::minimize(None),
            &NativeItem::maximize(Some("Zoom")),
            &NativeItem::separator(),
            &MenuItem::with_id(
                "previous-tab",
                "Show Previous Tab",
                true,
                Some(Accelerator::new(shift_command, Code::BracketLeft)),
            ),
            &MenuItem::with_id(
                "next-tab",
                "Show Next Tab",
                true,
                Some(Accelerator::new(shift_command, Code::BracketRight)),
            ),
            &NativeItem::separator(),
            &NativeItem::bring_all_to_front(None),
        ],
    )?;
    let help = Submenu::with_items(
        "Help",
        true,
        &[&MenuItem::with_id(
            "shortcuts",
            "Keyboard Shortcuts",
            true,
            None,
        )],
    )?;

    let menu = Menu::with_items(&[&app, &file, &edit, &view, &window, &help])?;
    menu.init_for_nsapp();
    window.set_as_windows_menu_for_nsapp();
    help.set_as_help_menu_for_nsapp();

    let ctx = ctx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let command = match event.id().0.as_str() {
            "settings" => Command::App(Action::Settings),
            "commands" => Command::App(Action::CommandPalette),
            "ask-ai" => Command::App(Action::AskAi),
            "shortcuts" => Command::App(Action::Shortcuts),
            "new-tab" => Command::App(Action::NewTab),
            "split-right" => Command::App(Action::SplitRight),
            "split-down" => Command::App(Action::SplitDown),
            "rename-session" => Command::App(Action::RenameSession),
            "close-tab" => Command::App(Action::CloseTab),
            "find" => Command::App(Action::Search),
            "sidebar" => Command::App(Action::ToggleSidebar),
            "increase-size" => Command::App(Action::ZoomIn),
            "decrease-size" => Command::App(Action::ZoomOut),
            "actual-size" => Command::App(Action::ResetZoom),
            "read-output" => Command::App(Action::ReadOutput),
            "previous-tab" => Command::App(Action::PreviousTab),
            "next-tab" => Command::App(Action::NextTab),
            "cut" => Command::Cut,
            "copy" => Command::Copy,
            "paste" => Command::Paste,
            _ => return,
        };
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<Vec<Command>>(queue_id())
                .push(command);
        });
        ctx.request_repaint();
    }));
    Ok(menu)
}

pub fn dispatch(ctx: &Context, mut perform: impl FnMut(Action)) {
    let commands = ctx.data_mut(|data| {
        data.remove_temp::<Vec<Command>>(queue_id())
            .unwrap_or_default()
    });
    for command in commands {
        // Send clipboard actions to egui so the focused text field or shell can handle them.
        let clipboard = match command {
            Command::App(action) => {
                perform(action);
                continue;
            }
            Command::Cut => ViewportCommand::RequestCut,
            Command::Copy => ViewportCommand::RequestCopy,
            Command::Paste => ViewportCommand::RequestPaste,
        };
        ctx.send_viewport_cmd(clipboard);
    }
}

fn queue_id() -> Id {
    Id::new("macos menu commands")
}

fn item(id: &str, label: &str, key: Code) -> MenuItem {
    MenuItem::with_id(
        id,
        label,
        true,
        Some(Accelerator::new(Modifiers::META, key)),
    )
}
