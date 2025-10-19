use eframe::egui::{Painter, Rect};

pub fn generate_tree_sprite(size: usize) -> egui::ColorImage {
    let mut img = egui::ColorImage::new([size, size], egui::Color32::TRANSPARENT);
    let cx = (size as f32) / 2.0;

    // Colors from SVG
    let foliage_col = egui::Color32::from_rgb(69, 188, 25);
    let trunk_col = egui::Color32::from_rgb(122, 70, 41);

    // Foliage - cloud-like blob at top (roughly 0-40% of height)
    let foliage_center_y = size as f32 * 0.28;
    let foliage_radius = size as f32 * 0.32;

    // Draw foliage as overlapping circles to create cloud shape
    blit_disc(&mut img, cx, foliage_center_y, foliage_radius, foliage_col);
    blit_disc(
        &mut img,
        cx - foliage_radius * 0.5,
        foliage_center_y + foliage_radius * 0.2,
        foliage_radius * 0.8,
        foliage_col,
    );
    blit_disc(
        &mut img,
        cx + foliage_radius * 0.5,
        foliage_center_y + foliage_radius * 0.2,
        foliage_radius * 0.8,
        foliage_col,
    );

    // Trunk - vertical line from foliage to bottom
    let trunk_width = (size as f32 * 0.08) as usize;
    let trunk_start_y = (foliage_center_y + foliage_radius * 0.5) as usize;
    let trunk_end_y = size - (size / 10); // Leave space for grass

    let trunk_x_start = (cx - trunk_width as f32 / 2.0) as usize;
    let trunk_x_end = (cx + trunk_width as f32 / 2.0) as usize;

    for y in trunk_start_y..trunk_end_y.min(size) {
        for x in trunk_x_start..trunk_x_end.min(size) {
            img[(x, y)] = trunk_col;
        }
    }

    // Small branches (simple angled lines)
    // Right branch
    let branch_y = trunk_start_y + (trunk_end_y - trunk_start_y) / 3;
    for i in 0..6 {
        let x = (cx + i as f32 * 1.2) as usize;
        let y = (branch_y as f32 - i as f32 * 0.8) as usize;
        if x < size && y < size {
            img[(x, y)] = trunk_col;
            if x + 1 < size {
                img[(x + 1, y)] = trunk_col;
            }
        }
    }

    // Left branch
    let branch_y2 = trunk_start_y + (trunk_end_y - trunk_start_y) * 2 / 3;
    for i in 0..7 {
        let x = (cx - i as f32 * 1.2) as usize;
        let y = (branch_y2 as f32 - i as f32 * 0.7) as usize;
        if x < size && y < size {
            img[(x, y)] = trunk_col;
            if x > 0 {
                img[(x - 1, y)] = trunk_col;
            }
        }
    }

    // Small grass patch at bottom
    let grass_height = size / 10;
    for y in (size - grass_height)..size {
        for x in trunk_x_start.saturating_sub(trunk_width * 2)
            ..trunk_x_end.saturating_add(trunk_width * 2).min(size)
        {
            img[(x, y)] = foliage_col;
        }
    }

    img
}

fn blit_disc(img: &mut egui::ColorImage, cx: f32, cy: f32, r: f32, col: egui::Color32) {
    let w = img.size[0] as i32;
    let h = img.size[1] as i32;
    let r2 = r * r;
    let (cx, cy) = (cx as i32, cy as i32);

    for y in (cy - r as i32).max(0)..(cy + r as i32).min(h - 1) {
        for x in (cx - r as i32).max(0)..(cx + r as i32).min(w - 1) {
            let dx = x - cx;
            let dy = y - cy;
            if (dx as f32 * dx as f32 + dy as f32 * dy as f32) <= r2 {
                img[(x as usize, y as usize)] = col;
            }
        }
    }
}

pub fn draw_tree_sprite(painter: &Painter, rect: Rect, tex: &egui::TextureHandle) {
    let r = rect.shrink(2.0);
    let scale = 1.1;
    let size = egui::vec2(r.width() * scale, r.height() * scale);
    let pos = egui::pos2(r.center().x - size.x / 2.0, r.bottom() - size.y);
    let dst = egui::Rect::from_min_size(pos, size);
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(tex.id(), dst, uv, egui::Color32::WHITE);
}
