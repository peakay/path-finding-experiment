use eframe::egui::{Painter, Rect};

pub fn draw_grass_tile(painter: &Painter, rect: Rect) {
    let base = egui::Color32::from_rgb(88, 160, 78);
    painter.rect_filled(rect.shrink(2.0), 2.0, base);
    let mut x = rect.left() + 3.0;
    while x < rect.right() - 3.0 {
        let h = 2.0 + ((x * 13.0).sin().abs() * 3.0);
        painter.line_segment(
            [
                egui::pos2(x, rect.bottom() - 3.0),
                egui::pos2(x + 1.0, rect.bottom() - 3.0 - h),
            ],
            egui::Stroke {
                width: 1.0,
                color: egui::Color32::from_rgb(120, 200, 110),
            },
        );
        x += 3.5;
    }
}

pub fn draw_water_tile(painter: &Painter, rect: Rect) {
    // Solid water color
    let base = egui::Color32::from_rgb(46, 105, 205);
    painter.rect_filled(rect.shrink(2.0), 2.0, base);
}

pub fn draw_sand_tile(painter: &Painter, rect: Rect) {
    // Simple, performant sand rendering - just a solid color with subtle variation
    let base = egui::Color32::from_rgb(220, 190, 150);
    painter.rect_filled(rect.shrink(2.0), 2.0, base);

    // Add just a few subtle dots for texture (much fewer than before)
    let center = rect.center();
    painter.circle_filled(center, 1.0, egui::Color32::from_rgb(200, 170, 140));

    // Only add corner dots if tile is large enough
    if rect.width() > 10.0 {
        let offset = rect.width() * 0.3;
        painter.circle_filled(
            egui::pos2(center.x - offset, center.y - offset),
            0.7,
            egui::Color32::from_rgb(210, 180, 145),
        );
        painter.circle_filled(
            egui::pos2(center.x + offset, center.y + offset),
            0.7,
            egui::Color32::from_rgb(210, 180, 145),
        );
    }
}

pub fn draw_wall_tile(painter: &Painter, rect: Rect) {
    let base = egui::Color32::from_rgb(90, 90, 95);
    painter.rect_filled(rect.shrink(1.0), 0.0, base);
    let r = rect.shrink(2.0);
    let bw = (r.width() / 3.0).max(2.0);
    let bh = (r.height() / 3.0).max(2.0);
    let mortar = egui::Color32::from_rgb(140, 140, 140);
    let stroke = egui::Stroke {
        width: 1.0,
        color: mortar,
    };
    let rows = (r.height() / bh).ceil() as i32;
    for row in 0..rows {
        let y = r.top() + row as f32 * bh;
        painter.line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], stroke);
        let offset = if row % 2 == 0 { 0.0 } else { bw / 2.0 };
        let mut x = r.left() + offset;
        while x <= r.right() {
            painter.line_segment(
                [egui::pos2(x, y), egui::pos2(x, (y + bh).min(r.bottom()))],
                stroke,
            );
            x += bw;
        }
    }
}
