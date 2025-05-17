use crate::app::GlobalAppState;
use anyhow::Result;

pub mod driver_station_setup;
pub mod student_starter_code;
pub mod system_firmware;

pub trait Page {
    fn run(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> Result<()>;

    fn get_title(&self) -> String;
}

fn add_next_button(ui: &mut egui::Ui, enabled: bool) -> egui::Response {
    add_custom_next_button(ui, "Next", enabled)
}

fn add_custom_next_button(
    ui: &mut egui::Ui,
    text: impl Into<egui::WidgetText>,
    enabled: bool,
) -> egui::Response {
    egui_alignments::Row::new(egui::Align::BOTTOM)
        .show(ui, |ui| -> egui::Response {
            egui_alignments::stretch(ui);
            ui.style_mut().text_styles.insert(
                egui::TextStyle::Button,
                egui::FontId::new(18.0, egui::FontFamily::Proportional),
            );
            let button_color = egui::Color32::from_hex(if enabled { "#71CC98" } else { "#A0A0A0" })
                .expect("Failed to parse color from hex.");
            ui.add_enabled(enabled, egui::Button::new(text).fill(button_color))
        })
        .inner
}
