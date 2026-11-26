//! Cards, labels, progress bars and the hand-drawn charts.

use super::*;
use chrono::NaiveDate;
use eframe::egui::{pos2, Shape};
use std::f32::consts::PI;

pub(super) fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::none()
        .fill(CARD)
        .rounding(12.0)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub(super) fn title(ui: &mut Ui, text: &str, sub: &str) {
    ui.label(RichText::new(text).size(26.0).strong());
    if !sub.is_empty() {
        ui.label(RichText::new(sub).color(MUTED));
    }
    ui.add_space(8.0);
}

pub(super) fn section(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(16.0).strong());
}

pub(super) fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(MUTED));
}

pub(super) fn primary(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(ACCENT)
            .rounding(8.0),
    )
}

pub(super) fn chip(ui: &mut Ui, text: &str, color: Color32) {
    Frame::none()
        .fill(color.linear_multiply(0.18))
        .rounding(10.0)
        .inner_margin(Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).size(12.0).strong());
        });
}

pub(super) fn progress(ui: &mut Ui, frac: f64, color: Color32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, height / 2.0, Color32::from_rgb(40, 52, 80));
    let f = frac.clamp(0.0, 1.0) as f32;
    if f > 0.0 {
        let mut r = rect;
        r.set_width((rect.width() * f).max(height));
        p.rect_filled(r, height / 2.0, color);
    }
}

pub(super) fn stat(ui: &mut Ui, label: &str, value: &str, sub: &str, color: Color32) {
    card(ui, |ui| {
        muted(ui, label);
        ui.label(RichText::new(value).size(26.0).strong());
        ui.label(RichText::new(sub).color(color).size(13.0));
    });
}

pub(super) fn amount_label(ui: &mut Ui, v: f64) {
    let (txt, c) = if v > 0.0 {
        (format!("+{}", money(v)), GREEN)
    } else {
        (money(-v), TEXT)
    };
    ui.label(RichText::new(txt).color(c).strong());
}

pub(super) fn alert_color(l: AlertLevel) -> Color32 {
    match l {
        AlertLevel::Info => BLUE,
        AlertLevel::Warn => AMBER,
        AlertLevel::Danger => ACCENT,
        AlertLevel::Good => GREEN,
    }
}

pub(super) fn budget_color(frac: f64) -> Color32 {
    if frac >= 1.0 {
        ACCENT
    } else if frac >= 0.85 {
        AMBER
    } else {
        GREEN
    }
}

pub(super) fn category_combo(ui: &mut Ui, id: &str, value: &mut Category, width: f32) {
    egui::ComboBox::new(id, "")
        .selected_text(format!("{} {}", value.icon(), value.name()))
        .width(width)
        .show_ui(ui, |ui| {
            for c in Category::all() {
                ui.selectable_value(value, c, format!("{} {}", c.icon(), c.name()));
            }
        });
}

/// The CoinCraft mark: a coral coin with a stacked smaller coin, drawn rather than loaded.
pub(super) fn logo(ui: &mut Ui, size: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    let c = r.center();
    let s = size / 34.0;
    p.circle_filled(c, 17.0 * s, ACCENT);
    p.circle_stroke(c, 12.0 * s, Stroke::new(1.5_f32 * s, Color32::from_rgb(255, 205, 190)));
    p.circle_filled(c + vec2(6.0 * s, -6.0 * s), 6.5 * s, Color32::from_rgb(255, 190, 150));
}

pub(super) fn money_field(ui: &mut Ui, v: &mut f64) {
    ui.add(egui::DragValue::new(v).speed(5.0).prefix("$").fixed_decimals(2));
    if *v < 0.0 {
        *v = 0.0;
    }
}

pub(super) fn in_days(d: NaiveDate) -> String {
    let n = (d - today()).num_days();
    match n {
        0 => "today".into(),
        1 => "tomorrow".into(),
        n if n < 0 => format!("{} days ago", -n),
        n => format!("in {} days", n),
    }
}

pub(super) fn alert_row(ui: &mut Ui, a: &Alert) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(4.0, 36.0), Sense::hover());
        ui.painter().rect_filled(r, 2.0, alert_color(a.level));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(a.title.as_str()).strong());
            ui.label(RichText::new(a.detail.as_str()).color(MUTED).size(12.5));
        });
    });
}

/// A label on the left and something pushed to the right edge.
pub(super) fn row_right(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        left(ui);
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

fn tooltip(p: &egui::Painter, at: egui::Pos2, txt: String, right_edge: f32) {
    let galley = p.layout_no_wrap(txt, FontId::proportional(12.0), TEXT);
    let at = pos2(at.x.min(right_edge - galley.size().x - 12.0), at.y);
    p.rect_filled(
        Rect::from_min_size(at, galley.size() + vec2(12.0, 8.0)),
        6.0,
        Color32::from_rgb(8, 12, 24),
    );
    p.galley(at + vec2(6.0, 4.0), galley, TEXT);
}

/// Income (green) and spending (coral) side by side for each month.
pub(super) fn cash_flow_chart(ui: &mut Ui, data: &[(String, f64, f64)], height: f32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    let max = data.iter().map(|d| d.1.max(d.2)).fold(1.0, f64::max) * 1.1;
    let plot = Rect::from_min_max(rect.min + vec2(48.0, 6.0), rect.max - vec2(0.0, 24.0));
    for i in 0..=4 {
        let y = plot.bottom() - plot.height() * i as f32 / 4.0;
        p.line_segment(
            [pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0_f32, Color32::from_rgb(34, 46, 74)),
        );
        p.text(
            pos2(plot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            money0(max * i as f64 / 4.0),
            FontId::proportional(11.0),
            MUTED,
        );
    }
    let slot = plot.width() / data.len().max(1) as f32;
    let bw = (slot * 0.28).min(26.0);
    let hover = resp.hover_pos();
    for (i, (label, inc, spend)) in data.iter().enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let hi = (*inc / max) as f32 * plot.height();
        let hs = (*spend / max) as f32 * plot.height();
        p.rect_filled(
            Rect::from_min_max(pos2(cx - bw - 2.0, plot.bottom() - hi), pos2(cx - 2.0, plot.bottom())),
            4.0,
            GREEN,
        );
        p.rect_filled(
            Rect::from_min_max(pos2(cx + 2.0, plot.bottom() - hs), pos2(cx + bw + 2.0, plot.bottom())),
            4.0,
            ACCENT,
        );
        p.text(
            pos2(cx, plot.bottom() + 12.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            MUTED,
        );
        if let Some(h) = hover {
            if (h.x - cx).abs() < slot / 2.0 && plot.contains(h) {
                let txt = format!(
                    "{}\nIn {}\nOut {}\nNet {}",
                    label,
                    money0(*inc),
                    money0(*spend),
                    money0(inc - spend)
                );
                tooltip(&p, pos2(cx + bw + 8.0, plot.top() + 4.0), txt, plot.right());
            }
        }
    }
}

pub(super) fn line_chart(ui: &mut Ui, data: &[(String, f64)], height: f32, color: Color32, fmt: fn(f64) -> String) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if data.len() < 2 {
        return;
    }
    let p = ui.painter_at(rect);
    let lo = data.iter().map(|d| d.1).fold(f64::MAX, f64::min);
    let hi = data.iter().map(|d| d.1).fold(f64::MIN, f64::max);
    let pad = ((hi - lo) * 0.15).max(1.0);
    let (lo, hi) = (lo - pad, hi + pad);
    let plot = Rect::from_min_max(rect.min + vec2(58.0, 8.0), rect.max - vec2(8.0, 24.0));
    for i in 0..=3 {
        let y = plot.bottom() - plot.height() * i as f32 / 3.0;
        p.line_segment(
            [pos2(plot.left(), y), pos2(plot.right(), y)],
            Stroke::new(1.0_f32, Color32::from_rgb(34, 46, 74)),
        );
        p.text(
            pos2(plot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            fmt(lo + (hi - lo) * i as f64 / 3.0),
            FontId::proportional(11.0),
            MUTED,
        );
    }
    let pts: Vec<egui::Pos2> = data
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let x = plot.left() + plot.width() * i as f32 / (data.len() - 1) as f32;
            let y = plot.bottom() - ((d.1 - lo) / (hi - lo)) as f32 * plot.height();
            pos2(x, y)
        })
        .collect();
    // Soft fill under the line, one strip at a time so each piece is convex.
    for w in pts.windows(2) {
        p.add(Shape::convex_polygon(
            vec![w[0], w[1], pos2(w[1].x, plot.bottom()), pos2(w[0].x, plot.bottom())],
            color.linear_multiply(0.12),
            Stroke::NONE,
        ));
    }
    p.add(Shape::line(pts.clone(), Stroke::new(2.5_f32, color)));
    let step = (data.len() / 6).max(1);
    for (i, d) in data.iter().enumerate() {
        if i % step == 0 || i == data.len() - 1 {
            p.text(
                pos2(pts[i].x, plot.bottom() + 12.0),
                Align2::CENTER_CENTER,
                d.0.as_str(),
                FontId::proportional(11.0),
                MUTED,
            );
        }
    }
    p.circle_filled(pts[pts.len() - 1], 5.0, color);
    if let Some(h) = resp.hover_pos() {
        let mut best = 0;
        for (i, pt) in pts.iter().enumerate() {
            if (pt.x - h.x).abs() < (pts[best].x - h.x).abs() {
                best = i;
            }
        }
        let pt = pts[best];
        p.line_segment([pos2(pt.x, plot.top()), pos2(pt.x, plot.bottom())], Stroke::new(1.0_f32, MUTED));
        p.circle_filled(pt, 5.0, color);
        tooltip(
            &p,
            pos2(pt.x + 8.0, plot.top()),
            format!("{}  {}", data[best].0, fmt(data[best].1)),
            plot.right(),
        );
    }
}

fn arc_points(c: egui::Pos2, r: f32, a0: f32, a1: f32) -> Vec<egui::Pos2> {
    let steps = (((a1 - a0).abs() / (PI / 60.0)).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

pub(super) fn donut(ui: &mut Ui, slices: &[(Color32, f64)], size: f32, top: &str, bottom: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let thick = size * 0.13;
    let r = size / 2.0 - thick / 2.0 - 2.0;
    let total: f64 = slices.iter().map(|s| s.1).sum();
    p.add(Shape::line(
        arc_points(c, r, 0.0, 2.0 * PI),
        Stroke::new(thick, Color32::from_rgb(40, 52, 80)),
    ));
    if total > 0.0 {
        let mut a = -PI / 2.0;
        for (color, v) in slices {
            let span = (*v / total) as f32 * 2.0 * PI;
            if span > 0.03 {
                p.add(Shape::line(arc_points(c, r, a + 0.01, a + span - 0.01), Stroke::new(thick, *color)));
            }
            a += span;
        }
    }
    p.text(c - vec2(0.0, 10.0), Align2::CENTER_CENTER, top, FontId::proportional(22.0), TEXT);
    p.text(
        c + vec2(0.0, 16.0),
        Align2::CENTER_CENTER,
        bottom,
        FontId::proportional(12.0),
        MUTED,
    );
}

pub(super) fn gauge(ui: &mut Ui, score: u16, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, width * 0.62), Sense::hover());
    let p = ui.painter();
    let c = pos2(rect.center().x, rect.bottom() - 16.0);
    let r = width * 0.42;
    let thick = 18.0_f32;
    let bands: [(u16, u16, Color32); 5] = [
        (300, 580, ACCENT),
        (580, 670, Color32::from_rgb(255, 159, 67)),
        (670, 740, AMBER),
        (740, 800, Color32::from_rgb(140, 220, 90)),
        (800, 850, GREEN),
    ];
    let ang = |s: u16| PI + (s.clamp(300, 850) - 300) as f32 / 550.0 * PI;
    for (a, b, col) in bands {
        p.add(Shape::line(
            arc_points(c, r, ang(a) + 0.01, ang(b) - 0.01),
            Stroke::new(thick, col.linear_multiply(0.3)),
        ));
    }
    for (a, b, col) in bands {
        if score > a {
            p.add(Shape::line(
                arc_points(c, r, ang(a) + 0.01, ang(score.min(b)) - 0.01),
                Stroke::new(thick, col),
            ));
        }
    }
    let a = ang(score);
    p.circle_filled(pos2(c.x + r * a.cos(), c.y + r * a.sin()), 11.0, TEXT);
    let (band, col) = score_band(score);
    p.text(
        c - vec2(0.0, 50.0),
        Align2::CENTER_CENTER,
        score.to_string(),
        FontId::proportional(46.0),
        TEXT,
    );
    p.text(c - vec2(0.0, 12.0), Align2::CENTER_CENTER, band, FontId::proportional(16.0), col);
    p.text(
        pos2(c.x - r, c.y + 4.0),
        Align2::CENTER_TOP,
        "300",
        FontId::proportional(11.0),
        MUTED,
    );
    p.text(
        pos2(c.x + r, c.y + 4.0),
        Align2::CENTER_TOP,
        "850",
        FontId::proportional(11.0),
        MUTED,
    );
}

/// Placeholder text for an empty field, dim so it can't be mistaken for something typed.
pub(super) fn hint(text: &str) -> RichText {
    RichText::new(text).color(Color32::from_rgb(92, 104, 132))
}
