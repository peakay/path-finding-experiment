use crate::agent::Agent;
use crate::animation::AnimationController;
use crate::events::EventQueue;
use crate::rendering::draw_log_entry;
use crate::tool_execution::ToolExecutionManager;
use eframe::egui;

/// Agent control panel
pub struct AgentPanel;

impl AgentPanel {
    /// Draw the activity log
    pub fn draw_activity_log(ui: &mut egui::Ui, agent: &Agent) {
        let scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true);

        scroll_area.show(ui, |ui| {
            for log_entry in agent.get_logs() {
                draw_log_entry(ui, log_entry);
            }
        });
    }

    /// Check if the agent is currently processing
    pub fn is_processing(
        event_queue: &EventQueue,
        tool_execution_manager: &ToolExecutionManager,
    ) -> bool {
        event_queue.pending_count() > 0 || tool_execution_manager.has_pending_executions()
    }

    /// Draw processing status indicator
    pub fn draw_processing_status(
        ui: &mut egui::Ui,
        event_queue: &EventQueue,
        animation_controller: &AnimationController,
    ) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("WORK").size(16.0).strong());
            ui.label(
                egui::RichText::new(
                    &animation_controller.get_processing_text(event_queue.pending_count()),
                )
                .color(egui::Color32::from_rgb(200, 100, 0))
                .strong(),
            );
        });
        ui.add_space(4.0);
    }

    /// Draw LLM thinking status indicator
    pub fn draw_thinking_status(ui: &mut egui::Ui, animation_controller: &AnimationController) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("AI").size(16.0).strong());
            ui.label(
                egui::RichText::new(&animation_controller.get_thinking_text())
                    .color(egui::Color32::from_rgb(100, 150, 255))
                    .strong(),
            );
        });
        ui.add_space(4.0);
    }
}
