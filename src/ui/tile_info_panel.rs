use crate::agent::Agent;
use crate::map::GridMap;
use eframe::egui;

/// Tile information panel
pub struct TileInfoPanel;

impl TileInfoPanel {
    pub fn draw(
        ui: &mut egui::Ui,
        selected_tile: Option<(usize, usize)>,
        map: &GridMap,
        agent: &Agent,
    ) {
        if let Some((tile_x, tile_y)) = selected_tile {
            ui.separator();
            ui.heading("Tile Info");

            egui::Frame::default()
                .fill(egui::Color32::from_rgb(245, 250, 255))
                .inner_margin(egui::Margin::same(8.0))
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("Position: ({}, {})", tile_x, tile_y))
                            .strong()
                            .color(egui::Color32::from_rgb(50, 80, 120)),
                    );

                    if let Some(tile_kind) = map.get(tile_x, tile_y) {
                        ui.horizontal(|ui| {
                            ui.label("Type:");
                            ui.label(
                                egui::RichText::new(tile_kind.name())
                                    .color(egui::Color32::from_rgb(80, 120, 80)),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label("Traversable:");
                            let (icon, color) = if tile_kind.is_traversable() {
                                ("YES", egui::Color32::from_rgb(50, 150, 50))
                            } else {
                                ("NO", egui::Color32::from_rgb(150, 50, 50))
                            };
                            ui.label(egui::RichText::new(icon).color(color).strong());
                        });

                        // Show if agent is on this tile
                        if tile_x == agent.x && tile_y == agent.y {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("AGENT").size(14.0).strong());
                                ui.label(
                                    egui::RichText::new(format!("{} is here", agent.name))
                                        .color(egui::Color32::from_rgb(200, 80, 50))
                                        .italics(),
                                );
                            });
                        }
                    }
                });
        }
    }
}
