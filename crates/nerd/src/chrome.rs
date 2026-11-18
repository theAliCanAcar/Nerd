use eframe::egui::{
    self, Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType,
    pos2, vec2,
};

use crate::theme::Palette;

#[derive(Clone, Copy)]
pub enum Icon {
    Terminal,
    Sidebar,
    Plus,
    Find,
    Ai,
    Settings,
    Commands,
    Output,
    Check,
    Close,
}

pub fn draw_icon(ui: &Ui, center: egui::Pos2, icon: Icon, color: Color32) {
    let painter = ui.painter();
    let point = |x, y| center + vec2(x, y);
    let stroke = Stroke::new(1.4, color);
    let line = |a, b| painter.line_segment([a, b], stroke);
    match icon {
        Icon::Terminal => {
            painter.rect_stroke(
                Rect::from_center_size(center, vec2(15.0, 12.0)),
                3,
                stroke,
                StrokeKind::Inside,
            );
            line(point(-4.0, -2.0), point(-1.5, 0.0));
            line(point(-1.5, 0.0), point(-4.0, 2.0));
            line(point(0.5, 2.0), point(4.0, 2.0));
        }
        Icon::Sidebar => {
            painter.rect_stroke(
                Rect::from_center_size(center, vec2(16.0, 13.0)),
                3,
                stroke,
                StrokeKind::Inside,
            );
            line(point(-2.5, -5.0), point(-2.5, 5.0));
        }
        Icon::Plus => {
            line(point(-5.0, 0.0), point(5.0, 0.0));
            line(point(0.0, -5.0), point(0.0, 5.0));
        }
        Icon::Find => {
            painter.circle_stroke(point(-1.5, -1.5), 4.4, stroke);
            line(point(1.7, 1.7), point(5.5, 5.5));
        }
        Icon::Ai => {
            painter.add(egui::Shape::closed_line(
                vec![
                    point(0.0, -7.0),
                    point(2.0, -2.0),
                    point(7.0, 0.0),
                    point(2.0, 2.0),
                    point(0.0, 7.0),
                    point(-2.0, 2.0),
                    point(-7.0, 0.0),
                    point(-2.0, -2.0),
                ],
                stroke,
            ));
        }
        Icon::Settings => {
            for y in [-4.0, 0.0, 4.0] {
                line(point(-6.0, y), point(6.0, y));
                painter.circle_filled(point(y * 0.65, y), 1.9, color);
            }
        }
        Icon::Commands => {
            painter.rect_stroke(
                Rect::from_center_size(center, vec2(16.0, 12.0)),
                3,
                stroke,
                StrokeKind::Inside,
            );
            line(point(-4.0, -2.0), point(4.0, -2.0));
            line(point(-4.0, 2.0), point(2.0, 2.0));
        }
        Icon::Check => {
            line(point(-4.0, 0.0), point(-1.0, 3.0));
            line(point(-1.0, 3.0), point(5.0, -3.0));
        }
        Icon::Output => {
            painter.rect_stroke(
                Rect::from_center_size(center, vec2(12.0, 14.0)),
                2,
                stroke,
                StrokeKind::Inside,
            );
            for y in [-3.0, 0.0, 3.0] {
                line(point(-3.0, y), point(3.0, y));
            }
        }
        Icon::Close => {
            line(point(-3.0, -3.0), point(3.0, 3.0));
            line(point(3.0, -3.0), point(-3.0, 3.0));
        }
    }
}

/// Paint the control using an ordinary egui click response for keyboard and accessibility input.
pub fn control(
    ui: &mut Ui,
    palette: &Palette,
    width: f32,
    icon: Icon,
    title: &str,
    selected: Option<bool>,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
    response.widget_info(|| match selected {
        Some(active) => WidgetInfo::selected(WidgetType::SelectableLabel, true, active, title),
        None => WidgetInfo::labeled(WidgetType::Button, true, title),
    });
    if ui.is_rect_visible(rect) {
        let active = selected == Some(true);
        let color = if active {
            palette.accent
        } else {
            palette.foreground
        };
        if active || matches!(icon, Icon::Ai) || response.hovered() || response.has_focus() {
            ui.painter().rect_filled(rect, 7, palette.accent_surface);
        }
        if response.hovered() || response.has_focus() {
            let outline = if response.has_focus() {
                palette.accent
            } else {
                palette.border
            };
            ui.painter()
                .rect_stroke(rect, 7, Stroke::new(1.25, outline), StrokeKind::Inside);
        }
        let icon_center = if width <= 36.0 {
            rect.center()
        } else {
            pos2(rect.left() + 16.0, rect.center().y)
        };
        draw_icon(ui, icon_center, icon, color);
        if width > 36.0 {
            let mut job = egui::text::LayoutJob::simple_singleline(
                title.to_owned(),
                if active {
                    crate::theme::emphasis(13.0)
                } else {
                    FontId::proportional(13.0)
                },
                color,
            );
            job.wrap.max_width = width - if selected.is_some() { 52.0 } else { 35.0 };
            job.wrap.max_rows = 1;
            let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
            let text = pos2(rect.left() + 32.0, rect.center().y - galley.size().y * 0.5);
            ui.painter().galley(text, galley, color);
            if active {
                draw_icon(
                    ui,
                    pos2(rect.right() - 12.0, rect.center().y),
                    Icon::Check,
                    color,
                );
            }
        }
    }
    response.on_hover_text(title)
}

pub fn draw_glasses(painter: &egui::Painter, center: egui::Pos2, scale: f32, color: Color32) {
    let at = |x: f32, y: f32| center + vec2(x * scale, y * scale);
    let frame = Stroke::new(2.15 * scale, color);
    let lens = |x: f32| {
        let rect = Rect::from_min_size(at(x, -5.6), vec2(10.6 * scale, 8.6 * scale));
        painter.rect_filled(rect, 2.4 * scale, color.gamma_multiply(0.22));
        painter.rect_stroke(rect, 2.4 * scale, frame, StrokeKind::Inside);
        let shine = Rect::from_min_size(
            rect.left_top() + vec2(1.3 * scale, 1.3 * scale),
            vec2(rect.width() * 0.42, 1.15 * scale),
        );
        painter.rect_filled(shine, 1.0 * scale, color.gamma_multiply(0.95));
    };
    lens(-12.2);
    lens(1.6);
    painter.line_segment([at(-1.6, -1.6), at(1.6, -1.6)], Stroke::new(2.3 * scale, color));
}

pub fn brand(ui: &mut Ui) {
    let palette = crate::theme::current(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, "Nerd"));
    let painter = ui.painter();
    painter.rect_filled(rect, 4, palette.terminal);
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, palette.border),
    );
    draw_glasses(painter, rect.left_top() + vec2(26.0, 28.0), 1.2, palette.accent);
    painter.text(
        rect.left_top() + vec2(54.0, 10.0),
        egui::Align2::LEFT_TOP,
        "Nerd",
        crate::theme::emphasis(16.0),
        palette.foreground,
    );
}
