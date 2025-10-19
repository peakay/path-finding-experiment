use crate::agent::LogEntry;
use eframe::egui;

/// Draw a rich log entry with color coding and formatting
pub fn draw_log_entry(ui: &mut egui::Ui, entry: &LogEntry) {
    let frame = egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
        .rounding(3.0);

    match entry {
        LogEntry::UserInstruction(text) => {
            frame
                .fill(egui::Color32::from_rgb(230, 240, 255))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("MSG").size(14.0).strong());
                        ui.label(
                            egui::RichText::new("You:")
                                .strong()
                                .color(egui::Color32::from_rgb(50, 100, 200)),
                        );
                    });
                    ui.label(egui::RichText::new(text).color(egui::Color32::from_rgb(40, 40, 60)));
                });
            ui.add_space(4.0);
        }
        LogEntry::AgentThinking(text) => {
            frame
                .fill(egui::Color32::from_rgb(245, 240, 255))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("AI").size(14.0).strong());
                        ui.label(
                            egui::RichText::new("Agent:")
                                .strong()
                                .color(egui::Color32::from_rgb(120, 80, 200)),
                        );
                    });
                    ui.label(
                        egui::RichText::new(text)
                            .color(egui::Color32::from_rgb(60, 40, 80))
                            .italics(),
                    );
                });
            ui.add_space(4.0);
        }
        LogEntry::ToolCall { name, args } => {
            frame
                .fill(egui::Color32::from_rgb(255, 250, 230))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("TOOL").size(14.0).strong());
                        ui.label(
                            egui::RichText::new("Tool Call:")
                                .strong()
                                .color(egui::Color32::from_rgb(180, 120, 20)),
                        );
                        ui.label(
                            egui::RichText::new(name)
                                .strong()
                                .color(egui::Color32::from_rgb(200, 100, 0)),
                        );
                    });
                    if !args.is_empty() {
                        ui.label(
                            egui::RichText::new(args)
                                .small()
                                .color(egui::Color32::from_rgb(100, 80, 40))
                                .font(egui::FontId::monospace(10.0)),
                        );
                    }
                });
            ui.add_space(4.0);
        }
        LogEntry::ToolProposal { name, data } => {
            // Rich tool execution proposals with custom UI per tool
            match name.as_str() {
                "think" => {
                    // Thinking tool - show as collapsible card with the agent's thoughts
                    let thoughts = data
                        .get("thoughts")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(no thoughts provided)");

                    frame
                        .fill(egui::Color32::from_rgb(250, 245, 255))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("THINK").size(16.0).strong());
                                ui.label(
                                    egui::RichText::new("Thinking")
                                        .strong()
                                        .size(13.0)
                                        .color(egui::Color32::from_rgb(130, 90, 180)),
                                );
                            });

                            ui.add_space(4.0);

                            // Show thoughts in a styled box
                            egui::Frame::default()
                                .fill(egui::Color32::from_rgb(255, 252, 255))
                                .inner_margin(egui::Margin::same(8.0))
                                .rounding(4.0)
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    egui::Color32::from_rgb(200, 180, 220),
                                ))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(thoughts)
                                            .color(egui::Color32::from_rgb(80, 60, 100))
                                            .italics(),
                                    );
                                });
                        });
                    ui.add_space(4.0);
                }
                "get_map_state" => {
                    // Map state tool - show parameters and indicate what area is being viewed
                    frame
                        .fill(egui::Color32::from_rgb(240, 248, 255))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("MAP").size(16.0).strong());
                                ui.label(
                                    egui::RichText::new("Viewing map")
                                        .color(egui::Color32::from_rgb(60, 100, 140)),
                                );
                            });

                            // Show what area is being viewed
                            if let Some(area) = data.get("area") {
                                if let (Some(x), Some(y)) = (area.get("x"), area.get("y")) {
                                    if let (Some(x_val), Some(y_val)) = (x.as_u64(), y.as_u64()) {
                                        ui.label(egui::RichText::new(format!("Area centered at ({}, {})", x_val, y_val))
                                            .small()
                                            .color(egui::Color32::from_rgb(80, 120, 160)));
                                    }
                                }
                            } else if let Some(visibility) = data.get("visibility") {
                                if let Some(vis_val) = visibility.as_u64() {
                                    ui.label(egui::RichText::new(format!("Visibility: {} tiles from agent", vis_val))
                                        .small()
                                        .color(egui::Color32::from_rgb(80, 120, 160)));
                                }
                            } else {
                                ui.label(egui::RichText::new("Full map view")
                                    .small()
                                    .color(egui::Color32::from_rgb(80, 120, 160)));
                            }
                        });
                    ui.add_space(4.0);
                }
                _ => {
                    // Fallback for unknown rich tools - show as generic tool call
                    frame
                        .fill(egui::Color32::from_rgb(255, 250, 230))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("TOOL").size(14.0).strong());
                                ui.label(
                                    egui::RichText::new("Tool Proposal:")
                                        .strong()
                                        .color(egui::Color32::from_rgb(180, 120, 20)),
                                );
                                ui.label(
                                    egui::RichText::new(name)
                                        .strong()
                                        .color(egui::Color32::from_rgb(200, 100, 0)),
                                );
                            });
                            ui.label(
                                egui::RichText::new(
                                    serde_json::to_string_pretty(data).unwrap_or_default(),
                                )
                                .small()
                                .color(egui::Color32::from_rgb(100, 80, 40))
                                .font(egui::FontId::monospace(10.0)),
                            );
                        });
                    ui.add_space(4.0);
                }
            }
        }
        LogEntry::ToolResult {
            name,
            success,
            message,
        } => {
            let (bg_color, icon, label_color) = if *success {
                (
                    egui::Color32::from_rgb(230, 255, 230),
                    "OK",
                    egui::Color32::from_rgb(50, 150, 50),
                )
            } else {
                (
                    egui::Color32::from_rgb(255, 235, 235),
                    "ERROR",
                    egui::Color32::from_rgb(200, 50, 50),
                )
            };

            frame.fill(bg_color).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(14.0));
                    ui.label(egui::RichText::new("Result:").strong().color(label_color));
                    ui.label(egui::RichText::new(name).color(label_color));
                });

                // Special handling for get_map_state results - show the minimap
                if name == "get_map_state" && *success {
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(message) {
                        if let Some(minimap) = json_value.get("_minimap").and_then(|m| m.as_str()) {
                            // Display the minimap in a monospace font
                            egui::ScrollArea::vertical()
                                .max_height(200.0) // Limit height to keep UI manageable
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(minimap)
                                            .font(egui::FontId::monospace(11.0))
                                            .color(egui::Color32::from_rgb(40, 80, 120)),
                                    );
                                });
                        } else {
                            // Fallback to showing the raw message if no minimap
                            ui.label(
                                egui::RichText::new(message)
                                    .small()
                                    .color(egui::Color32::from_gray(80)),
                            );
                        }
                    } else {
                        // Fallback if JSON parsing fails
                        ui.label(
                            egui::RichText::new(message)
                                .small()
                                .color(egui::Color32::from_gray(80)),
                        );
                    }
                } else {
                    // Default display for other tool results
                    ui.label(
                        egui::RichText::new(message)
                            .small()
                            .color(egui::Color32::from_gray(80)),
                    );
                }
            });
            ui.add_space(4.0);
        }
        LogEntry::Movement {
            direction,
            position,
        } => {
            frame
                .fill(egui::Color32::from_rgb(240, 255, 240))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let arrow = match direction.as_str() {
                            "up" => "^",
                            "down" => "v",
                            "left" => "<",
                            "right" => ">",
                            _ => ">",
                        };
                        ui.label(egui::RichText::new(arrow).size(14.0));
                        ui.label(
                            egui::RichText::new("Move:")
                                .strong()
                                .color(egui::Color32::from_rgb(80, 150, 80)),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} → ({}, {})",
                                direction, position.0, position.1
                            ))
                            .color(egui::Color32::from_rgb(60, 120, 60)),
                        );
                    });
                });
            ui.add_space(4.0);
        }
        LogEntry::Error(text) => {
            frame
                .fill(egui::Color32::from_rgb(255, 220, 220))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("WARN").size(16.0).strong());
                        ui.label(
                            egui::RichText::new("Error:")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 50, 50)),
                        );
                    });

                    // Split error text into lines for better readability
                    let lines: Vec<&str> = text.split('\n').collect();
                    for (i, line) in lines.iter().enumerate() {
                        if i == 0 {
                            ui.label(
                                egui::RichText::new(*line)
                                    .color(egui::Color32::from_rgb(180, 40, 40))
                                    .strong(),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(*line)
                                    .color(egui::Color32::from_rgb(160, 60, 60))
                                    .small(),
                            );
                        }
                    }
                });
            ui.add_space(4.0);
        }
        LogEntry::Info(text) => {
            frame
                .fill(egui::Color32::from_rgb(245, 245, 250))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("INFO").size(14.0).strong());
                        ui.label(egui::RichText::new(text).color(egui::Color32::from_gray(100)));
                    });
                });
            ui.add_space(4.0);
        }
    }
}
