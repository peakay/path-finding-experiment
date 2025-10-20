use crate::agent::Agent;
use crate::map::{GridMap, TileKind};
use crate::rendering::*;
use eframe::egui;
use egui::{Painter, Rect};

/// Handles rendering of the game board
pub struct BoardRenderer;

impl BoardRenderer {
    /// Render the game board with tiles, grid, agent, and trail
    pub fn render(
        painter: &Painter,
        rect: Rect,
        map: &GridMap,
        agent: &Agent,
        selected_cell: Option<(usize, usize)>,
        tree_tex: Option<&egui::TextureHandle>,
    ) {
        let board_dim = map.width().max(map.height());
        let n = board_dim as f32;
        let cell = rect.width() / n;

        // Background
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(240, 240, 240));

        // Grid lines
        let line_color = egui::Color32::from_gray(180);
        let stroke = egui::Stroke {
            width: 1.0,
            color: line_color,
        };
        for i in 0..=board_dim {
            let x = rect.left() + (i as f32) * cell;
            let y = rect.top() + (i as f32) * cell;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                stroke,
            );
        }

        // Paint tiles
        for y in 0..map.height() {
            for x in 0..map.width() {
                let x0 = rect.left() + (x as f32) * cell;
                let y0 = rect.top() + (y as f32) * cell;
                let rcell = egui::Rect::from_min_size(egui::pos2(x0, y0), egui::vec2(cell, cell));
                if let Some(kind) = map.get(x, y) {
                    match kind {
                        TileKind::Empty => {}
                        TileKind::Grass => draw_grass_tile(painter, rcell),
                        TileKind::Water => draw_water_tile(painter, rcell),
                        TileKind::Sand => draw_sand_tile(painter, rcell),
                        TileKind::Wall => draw_wall_tile(painter, rcell),
                        TileKind::Trail => {
                            // Legacy trail tile - should not exist in new system
                            painter.rect_filled(
                                rcell.shrink(4.0),
                                2.0,
                                egui::Color32::from_rgba_premultiplied(255, 200, 0, 100),
                            );
                        }
                        TileKind::Tree => {
                            if let Some(tex) = tree_tex {
                                draw_tree_sprite(painter, rcell, tex);
                            } else {
                                draw_grass_tile(painter, rcell);
                            }
                        }
                        TileKind::Custom(code) => {
                            let r = ((code >> 16) & 0xFF) as u8;
                            let g = ((code >> 8) & 0xFF) as u8;
                            let b = (code & 0xFF) as u8;
                            painter.rect_filled(
                                rcell.shrink(2.0),
                                0.0,
                                egui::Color32::from_rgb(r, g, b),
                            );
                        }
                    }
                }
            }
        }

        // Draw agent trail based on movement history
        for &(trail_x, trail_y) in agent.get_movement_history() {
            if trail_x < map.width() && trail_y < map.height() {
                let x0 = rect.left() + (trail_x as f32) * cell;
                let y0 = rect.top() + (trail_y as f32) * cell;
                let rcell = egui::Rect::from_min_size(egui::pos2(x0, y0), egui::vec2(cell, cell));
                painter.rect_filled(
                    rcell.shrink(4.0),
                    2.0,
                    egui::Color32::from_rgba_premultiplied(255, 200, 0, 100),
                );
            }
        }

        // Selection highlight
        if let Some((sr, sc)) = selected_cell {
            let x0 = rect.left() + (sc as f32) * cell;
            let y0 = rect.top() + (sr as f32) * cell;
            let rcell = egui::Rect::from_min_size(egui::pos2(x0, y0), egui::vec2(cell, cell));
            painter.rect_stroke(
                rcell.shrink(1.0),
                0.0,
                egui::Stroke {
                    width: 2.0,
                    color: egui::Color32::YELLOW,
                },
            );
        }

        // Draw agent
        if agent.x < map.width() && agent.y < map.height() {
            let x0 = rect.left() + (agent.x as f32) * cell;
            let y0 = rect.top() + (agent.y as f32) * cell;
            let center = egui::pos2(x0 + cell * 0.5, y0 + cell * 0.6);
            painter.circle_filled(center, cell * 0.18, egui::Color32::from_rgb(230, 70, 50));
            painter.text(
                egui::pos2(center.x, y0 + cell * 0.15),
                egui::Align2::CENTER_CENTER,
                &agent.name,
                egui::FontId::proportional((cell * 0.32).max(10.0)),
                egui::Color32::BLACK,
            );
        }
    }

    /// Convert screen position to grid coordinates (hit testing)
    pub fn screen_to_grid(
        pos: egui::Pos2,
        rect: Rect,
        board_side: f32,
        board_dim: usize,
    ) -> Option<(usize, usize)> {
        let cell = board_side / (board_dim as f32);
        let rel_x = (pos.x - rect.left()).clamp(0.0, board_side - 0.001);
        let rel_y = (pos.y - rect.top()).clamp(0.0, board_side - 0.001);
        let c = (rel_x / cell).floor() as usize;
        let r = (rel_y / cell).floor() as usize;

        if r < board_dim && c < board_dim {
            Some((r, c))
        } else {
            None
        }
    }
}
