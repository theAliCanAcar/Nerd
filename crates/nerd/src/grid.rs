use nerd_terminal::{Cell, Color, CursorShape, Snapshot};
use eframe::egui::{self, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use std::ops::RangeInclusive;

use crate::theme::Palette;

pub type CellPosition = (usize, usize);

#[derive(Clone, Copy)]
pub struct Selection {
    pub anchor: CellPosition,
    pub end: CellPosition,
}

impl Selection {
    pub fn range(self) -> RangeInclusive<CellPosition> {
        self.anchor.min(self.end)..=self.anchor.max(self.end)
    }
}

pub struct Geometry {
    pub rect: Rect,
    pub width: f32,
    pub height: f32,
    pub text_offset: f32,
    pub cols: u16,
    pub rows: u16,
    pub font: FontId,
}

impl Geometry {
    pub fn new(ui: &Ui, rect: Rect, font_size: f32) -> Self {
        let font = FontId::monospace(font_size);
        let (width, line_height) =
            ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'M'), fonts.row_height(&font)));
        let scale = ui.ctx().pixels_per_point();
        let width = (width * scale).round().max(1.0) / scale;
        let height = ((line_height + 3.0) * scale).round() / scale;
        let rect = rect.shrink2(egui::vec2(15.0, 12.0));
        Self {
            rect,
            width,
            height,
            text_offset: (height - line_height) / 2.0,
            cols: (rect.width() / width).floor().clamp(2.0, 512.0) as u16,
            rows: (rect.height() / height).floor().clamp(2.0, 256.0) as u16,
            font,
        }
    }

    pub fn cell(&self, position: Pos2) -> CellPosition {
        let relative = (position - self.rect.min).max(Vec2::ZERO);
        let row = (relative.y / self.height) as usize;
        let col = (relative.x / self.width) as usize;
        (
            row.min(self.rows as usize - 1),
            col.min(self.cols as usize - 1),
        )
    }

    fn bounds(&self, row: usize, col: usize, wide: bool) -> Rect {
        let pos = self.rect.min + egui::vec2(col as f32 * self.width, row as f32 * self.height);
        Rect::from_min_size(
            pos,
            egui::vec2(if wide { 2.0 } else { 1.0 } * self.width, self.height),
        )
    }
}

pub fn paint(
    painter: &Painter,
    grid: &Geometry,
    snapshot: &Snapshot,
    palette: &Palette,
    selection: Option<Selection>,
    search_match: Option<(usize, usize, usize)>,
    focused: bool,
) {
    let selected = selection.map(Selection::range);
    // Paint backgrounds first so each wide glyph can extend into its trailing cell.
    for (index, cell) in snapshot.cells.iter().enumerate() {
        let (row, col) = (index / snapshot.cols, index % snapshot.cols);
        let rect = grid.bounds(row, col, false);
        let (_, bg) = colors(cell, palette);
        let highlight = selected.as_ref().is_some_and(|range| {
            range.contains(&(row, col))
                || (cell.spacer && col > 0 && range.contains(&(row, col - 1)))
                || (cell.wide && range.contains(&(row, col + 1)))
        });
        let found = search_match.is_some_and(|(r, c, len)| row == r && (c..c + len).contains(&col));
        let bg = if highlight {
            palette.selection
        } else if found {
            palette.accent_surface
        } else {
            bg
        };
        if bg != palette.terminal {
            painter.rect_filled(rect, 0.0, bg);
        }
    }
    for (index, cell) in snapshot.cells.iter().enumerate() {
        if cell.hidden || cell.spacer {
            continue;
        }
        let (row, col) = (index / snapshot.cols, index % snapshot.cols);
        let rect = grid.bounds(row, col, cell.wide);
        let (fg, _) = colors(cell, palette);
        draw_cell(painter, grid, rect, cell, fg);
    }

    if let Some((row, col)) = snapshot.cursor.filter(|_| !snapshot.exited) {
        let cell = snapshot.cells.get(row * snapshot.cols + col);
        let rect = grid.bounds(row, col, cell.is_some_and(|c| c.wide));
        let accent = palette.accent;
        match (focused, snapshot.cursor_shape) {
            (_, CursorShape::Hidden) => {}
            (false, _) | (_, CursorShape::HollowBlock) => {
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, accent), StrokeKind::Inside);
            }
            (_, CursorShape::Block) => {
                painter.rect_filled(rect, 0.0, accent);
                if let Some(cell) = cell.filter(|cell| !cell.hidden && !cell.spacer) {
                    draw_cell(painter, grid, rect, cell, palette.terminal);
                }
            }
            (_, CursorShape::Beam) => {
                painter.rect_filled(
                    Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height())),
                    0.0,
                    accent,
                );
            }
            (_, CursorShape::Underline) => {
                painter.rect_filled(
                    Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 2.0), rect.max),
                    0.0,
                    accent,
                );
            }
        }
    }
}

pub fn agent_rect(grid: &Geometry, cursor: (usize, usize)) -> Rect {
    let origin = grid.rect.min
        + egui::vec2(
            (cursor.1 as f32 + 1.0) * grid.width + 6.0,
            cursor.0 as f32 * grid.height,
        );
    Rect::from_min_size(origin, egui::vec2(34.0, grid.height.max(14.0)))
}

pub fn paint_agent(painter: &Painter, rect: Rect, color: Color32, time: f64) {
    let mid_y = rect.center().y;
    for index in 0..3 {
        let phase = time * 4.2 + index as f64 * 0.85;
        let pulse = 0.45 + 0.55 * ((phase.sin() * 0.5) + 0.5);
        let center = Pos2::new(rect.left() + 6.0 + index as f32 * 10.0, mid_y);
        star(painter, center, 3.2 + pulse as f32, color.gamma_multiply(pulse as f32));
    }
}

fn star(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let mut points = Vec::with_capacity(10);
    for step in 0..10 {
        let arm = if step % 2 == 0 { radius } else { radius * 0.42 };
        let angle = (step as f32) * (std::f32::consts::PI / 5.0) - std::f32::consts::FRAC_PI_2;
        points.push(center + egui::vec2(angle.cos() * arm, angle.sin() * arm));
    }
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        Stroke::NONE,
    ));
}

pub fn plain_text(snapshot: &Snapshot) -> String {
    let mut output = String::new();
    let mut line = String::new();
    for row in snapshot.cells.chunks(snapshot.cols.max(1)) {
        line.clear();
        for cell in row.iter().filter(|cell| !cell.spacer) {
            if cell.hidden || cell.text.is_empty() {
                line.push(' ');
                if cell.wide {
                    line.push(' ');
                }
            } else {
                line.extend(cell.text.chars().filter(|ch| !ch.is_control()));
            }
        }
        output.push_str(line.trim_end_matches(' '));
        output.push('\n');
    }
    output.truncate(output.trim_end_matches('\n').len());
    output
}

fn colors(cell: &Cell, palette: &Palette) -> (Color32, Color32) {
    let mut fg = match cell.fg {
        Color::Indexed(i @ 0..=7) if cell.bold => palette.indexed(i + 8),
        color => resolve(color, palette.foreground, palette),
    };
    let mut bg = resolve(cell.bg, palette.terminal, palette);
    if cell.inverse {
        std::mem::swap(&mut fg, &mut bg);
    }
    if cell.dim {
        fg = fg.lerp_to_gamma(bg, 0.37);
    }
    (fg, bg)
}

fn resolve(color: Color, default: Color32, palette: &Palette) -> Color32 {
    match color {
        Color::Default => default,
        Color::Indexed(index) => palette.indexed(index),
        Color::Rgb([r, g, b]) => Color32::from_rgb(r, g, b),
    }
}

fn draw_cell(painter: &Painter, grid: &Geometry, rect: Rect, cell: &Cell, fg: Color32) {
    if !cell.text.is_empty() && cell.text != " " {
        let galley = if cell.italic {
            let mut job = egui::text::LayoutJob::default();
            job.append(
                &cell.text,
                0.0,
                egui::TextFormat {
                    font_id: grid.font.clone(),
                    color: fg,
                    italics: true,
                    ..Default::default()
                },
            );
            painter.layout_job(job)
        } else {
            painter.layout_no_wrap(cell.text.clone(), grid.font.clone(), fg)
        };
        let pos = rect.min + egui::vec2(0.0, grid.text_offset);
        painter.galley(pos, galley.clone(), fg);
        // The bundled monospace face is regular weight. A subpixel offset keeps bold legible.
        if cell.bold {
            painter.galley(pos + egui::vec2(0.55, 0.0), galley, fg);
        }
    }
    if cell.underline {
        painter.hline(rect.x_range(), rect.bottom() - 2.0, Stroke::new(1.0, fg));
    }
    if cell.strikeout {
        painter.hline(rect.x_range(), rect.center().y, Stroke::new(1.0, fg));
    }
}

pub fn word_selection(snapshot: &Snapshot, (row, col): CellPosition) -> Selection {
    let cols = snapshot.cols.max(1);
    let col = col.min(cols - 1);
    let start_word = is_word(cell_char(snapshot, row, col));
    let mut left = col;
    let mut right = col;
    while left > 0 && is_word(cell_char(snapshot, row, left - 1)) == start_word {
        left -= 1;
    }
    while right + 1 < cols && is_word(cell_char(snapshot, row, right + 1)) == start_word {
        right += 1;
    }
    Selection { anchor: (row, left), end: (row, right) }
}

pub fn line_selection(snapshot: &Snapshot, row: usize) -> Selection {
    let cols = snapshot.cols.max(1);
    let mut end = 0;
    for col in 0..cols {
        if !cell_char(snapshot, row, col).is_whitespace() {
            end = col;
        }
    }
    Selection { anchor: (row, 0), end: (row, end) }
}

pub fn screen_selection(snapshot: &Snapshot) -> Selection {
    Selection {
        anchor: (0, 0),
        end: (snapshot.rows.saturating_sub(1), snapshot.cols.saturating_sub(1)),
    }
}

fn cell_char(snapshot: &Snapshot, row: usize, col: usize) -> char {
    snapshot
        .cells
        .get(row * snapshot.cols + col)
        .and_then(|cell| cell.text.chars().next())
        .filter(|ch| !ch.is_control())
        .unwrap_or(' ')
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '~' | '@')
}
