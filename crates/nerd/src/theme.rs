use eframe::Storage;
use eframe::egui::{
    self, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Stroke, ThemePreference,
    Visuals,
};

pub fn emphasis(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("inter-semibold".into()))
}

pub const FONT_SIZE: f32 = 15.0;
pub const MIN_FONT_SIZE: f32 = 9.0;
pub const MAX_FONT_SIZE: f32 = 32.0;

const THEME_KEY: &str = "appearance.theme";
const FONT_SIZE_KEY: &str = "appearance.font_size";
const SIDEBAR_KEY: &str = "appearance.sidebar";

pub fn load_sidebar(storage: Option<&dyn Storage>) -> bool {
    storage
        .and_then(|storage| storage.get_string(SIDEBAR_KEY))
        .as_deref()
        != Some("hidden")
}

pub fn save_sidebar(storage: &mut dyn Storage, visible: bool) {
    storage.set_string(SIDEBAR_KEY, if visible { "shown" } else { "hidden" }.into());
}

pub fn load_appearance(storage: Option<&dyn Storage>) -> (ThemePreference, f32) {
    let preference = match storage
        .and_then(|storage| storage.get_string(THEME_KEY))
        .as_deref()
    {
        Some("light") => ThemePreference::Light,
        Some("dark") => ThemePreference::Dark,
        _ => ThemePreference::System,
    };
    let font_size = storage
        .and_then(|storage| storage.get_string(FONT_SIZE_KEY))
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|size| (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(size))
        .unwrap_or(FONT_SIZE);
    (preference, font_size)
}

pub fn save_appearance(storage: &mut dyn Storage, preference: ThemePreference, font_size: f32) {
    let theme = match preference {
        ThemePreference::System => "system",
        ThemePreference::Light => "light",
        ThemePreference::Dark => "dark",
    };
    storage.set_string(THEME_KEY, theme.to_owned());
    storage.set_string(FONT_SIZE_KEY, font_size.to_string());
}

pub struct Palette {
    pub chrome: Color32,
    pub sidebar: Color32,
    pub workspace: Color32,
    pub terminal: Color32,
    pub foreground: Color32,
    pub muted: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub accent_surface: Color32,
    pub selection: Color32,
    pub error: Color32,
    pub warning_surface: Color32,
    pub action: Color32,
    pub action_text: Color32,
    ansi: [Color32; 16],
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

const DARK: Palette = Palette {
    chrome: hex(0x202726),
    sidebar: hex(0x171e1d),
    workspace: hex(0x191f1e),
    terminal: hex(0x111716),
    foreground: hex(0xe9ece8),
    muted: hex(0xa5ada6),
    border: hex(0x36443e),
    accent: hex(0x8cd6b9),
    accent_surface: hex(0x233c32),
    selection: hex(0x295644),
    error: hex(0xf2a198),
    warning_surface: hex(0x372c22),
    action: hex(0x9adcbf),
    action_text: hex(0x102219),
    ansi: [
        hex(0x111716),
        hex(0xec897f),
        hex(0x8bcea7),
        hex(0xe0c990),
        hex(0x8aaff0),
        hex(0xcaa6d9),
        hex(0x88ccd0),
        hex(0xd5dbd6),
        hex(0x929d95),
        hex(0xffa199),
        hex(0xa3e5bb),
        hex(0xf6dda4),
        hex(0xa9c5fc),
        hex(0xdec0e9),
        hex(0xa2e2e4),
        hex(0xf1f4f0),
    ],
};

const LIGHT: Palette = Palette {
    chrome: hex(0xfcfcf9),
    sidebar: hex(0xf0f2ed),
    workspace: hex(0xf4f5f0),
    terminal: hex(0xfcfcfb),
    foreground: hex(0x1a1a17),
    muted: hex(0x646961),
    border: hex(0xdce2d9),
    accent: hex(0x1f6f5c),
    accent_surface: hex(0xeaf2ef),
    selection: hex(0xcce5d9),
    error: hex(0xab392e),
    warning_surface: hex(0xfaefe1),
    action: hex(0x165746),
    action_text: hex(0xf8fffb),
    ansi: [
        hex(0x252522),
        hex(0xad3731),
        hex(0x286e4a),
        hex(0x80580f),
        hex(0x2a58a0),
        hex(0x793c8d),
        hex(0x1d6871),
        hex(0x626762),
        hex(0x646b65),
        hex(0xb73737),
        hex(0x13724b),
        hex(0x846320),
        hex(0x2b5eaf),
        hex(0x8b408a),
        hex(0x176f77),
        hex(0x343934),
    ],
};

impl Palette {
    pub fn indexed(&self, index: u8) -> Color32 {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let level = |i| if i == 0 { 0 } else { 55 + i * 40 };
                let i = index - 16;
                Color32::from_rgb(level(i / 36), level(i % 36 / 6), level(i % 6))
            }
            _ => Color32::from_gray(8 + (index - 232) * 10),
        }
    }
}

pub fn current(ctx: &Context) -> &'static Palette {
    match ctx.theme() {
        egui::Theme::Dark => &DARK,
        egui::Theme::Light => &LIGHT,
    }
}

pub fn configure(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            "inter-regular",
            include_bytes!("../../../assets/fonts/Inter-Regular.ttf").as_slice(),
        ),
        (
            "inter-semibold",
            include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), FontData::from_static(bytes).into());
    }
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "inter-regular".into());
    let mut semibold = proportional.clone();
    semibold[0] = "inter-semibold".into();
    fonts
        .families
        .insert(FontFamily::Name("inter-semibold".into()), semibold);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style
            .text_styles
            .insert(egui::TextStyle::Heading, emphasis(21.0));
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.item_spacing.y = 6.0;
    });
    for (theme, base, palette) in [
        (egui::Theme::Dark, Visuals::dark(), &DARK),
        (egui::Theme::Light, Visuals::light(), &LIGHT),
    ] {
        let mut visuals = base;
        visuals.panel_fill = palette.terminal;
        visuals.window_fill = palette.chrome;
        visuals.extreme_bg_color = palette.terminal;
        visuals.faint_bg_color = palette.chrome;
        visuals.override_text_color = Some(palette.foreground);
        visuals.selection.bg_fill = palette.selection;
        visuals.selection.stroke = Stroke::new(1.0, palette.accent);
        visuals.hyperlink_color = palette.accent;
        visuals.error_fg_color = palette.error;
        visuals.window_stroke = Stroke::new(1.0, palette.border);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
        visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
        visuals.widgets.inactive.bg_fill = palette.chrome;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
        visuals.widgets.hovered.bg_fill = palette.accent_surface;
        visuals.widgets.hovered.weak_bg_fill = palette.accent_surface;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
        visuals.widgets.active.bg_stroke = Stroke::new(1.5, palette.accent);
        visuals.widgets.active.weak_bg_fill = palette.accent_surface;
        ctx.set_visuals_of(theme, visuals);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum CursorStyle {
    #[default]
    Shell,
    Block,
    Beam,
    Underline,
}

impl CursorStyle {
    pub const OPTIONS: [Self; 4] = [Self::Shell, Self::Block, Self::Beam, Self::Underline];
    const KEY: &str = "appearance.cursor";

    pub fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell default",
            Self::Block => "Block",
            Self::Beam => "Beam",
            Self::Underline => "Underline",
        }
    }

    pub fn load(storage: Option<&dyn Storage>) -> Self {
        match storage.and_then(|s| s.get_string(Self::KEY)).as_deref() {
            Some("Block") => Self::Block,
            Some("Beam") => Self::Beam,
            Some("Underline") => Self::Underline,
            _ => Self::Shell,
        }
    }

    pub fn save(self, storage: &mut dyn Storage) {
        storage.set_string(Self::KEY, self.label().into());
    }

    pub fn resolve(self, shell: nerd_terminal::CursorShape) -> nerd_terminal::CursorShape {
        use nerd_terminal::CursorShape;
        if shell == CursorShape::Hidden {
            return shell;
        }
        match self {
            Self::Shell => shell,
            Self::Block => CursorShape::Block,
            Self::Beam => CursorShape::Beam,
            Self::Underline => CursorShape::Underline,
        }
    }
}
