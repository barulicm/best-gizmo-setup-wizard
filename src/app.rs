use eframe::{App, Frame};

pub struct GlobalAppState {
    pub tmp_dir: tempfile::TempDir,
}

pub struct MyApp {
    current_page: Option<Box<dyn crate::pages::Page>>,
    state: GlobalAppState,
    page_error: Option<anyhow::Error>,
}

impl MyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let tmp_dir = tempfile::Builder::new()
            .prefix("best-gizmo-setup-wizard")
            .tempdir()
            .expect("Failed to create temporary directory");
        Self {
            current_page: None,
            state: GlobalAppState { tmp_dir },
            page_error: None,
        }
    }

    fn run_start_page(&mut self, ui: &mut egui::Ui) {
        egui_alignments::column(ui, egui::Align::LEFT, |ui| {
            egui_alignments::row(ui, egui::Align::TOP, |ui| {
                egui_alignments::stretch(ui);
                ui.heading("BEST Gizmo Software Installer");
                egui_alignments::stretch(ui);
            });
            ui.label("This tool will help you install or update your Gizmo software.");
            ui.label(
                "Select which software you would like to install, then follow the instructions.",
            );

            egui_alignments::stretch(ui);

            egui_alignments::row(ui, egui::Align::Center, |ui| {
                egui_alignments::stretch(ui);

                egui_alignments::column(ui, egui::Align::Center, |ui| {
                    let button =
                        egui::ImageButton::new(egui::include_image!("assets/driver_station.png"));
                    if ui.add_sized([150.0, 150.0], button).clicked() {
                        self.current_page = Some(Box::new(
                            crate::pages::driver_station_setup::DriverStationSetupPage::new(),
                        ));
                    }
                    ui.label("Driver Station");
                });

                egui_alignments::column(ui, egui::Align::Center, |ui| {
                    let button = egui::ImageButton::new(egui::include_image!(
                        "assets/gizmo_system_processor.png"
                    ));
                    if ui.add_sized([150.0, 150.0], button).clicked() {
                        self.current_page = Some(Box::new(
                            crate::pages::system_firmware::SystemFirmwarePage::new(),
                        ));
                    }
                    ui.label("System Firmware");
                });

                egui_alignments::column(ui, egui::Align::Center, |ui| {
                    ui.disable();
                    let button = egui::ImageButton::new(egui::include_image!(
                        "assets/gizmo_student_processor.png"
                    ));
                    if ui.add_sized([150.0, 150.0], button).clicked() {
                        self.current_page = Some(Box::new(
                            crate::pages::student_starter_code::StudentStarterCodePage::new(),
                        ));
                    }
                    ui.label("Coming Soon");
                });

                egui_alignments::stretch(ui);
            });
        });
    }

    fn add_top_panel(&mut self, ctx: &egui::Context) {
        let top_panel_frame = egui::containers::Frame::new()
            .fill(egui::Color32::from_hex("#001E62").expect("Failed to parse color from hex."))
            .inner_margin(10);
        egui::TopBottomPanel::top("top_panel")
            .frame(top_panel_frame)
            .show(ctx, |ui| {
                egui_extras::StripBuilder::new(ui)
                    .sizes(egui_extras::Size::relative(0.33), 3)
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            ui.style_mut().text_styles.insert(
                                egui::TextStyle::Button,
                                egui::FontId::new(14.0, egui::FontFamily::Proportional),
                            );
                            let icon = egui::include_image!(
                                "../src/assets/icons/ic_fluent_arrow_hook_up_left_28_filled.svg"
                            );
                            let start_over_button =
                                egui::Button::image_and_text(icon, "Start Over")
                                    .wrap_mode(egui::TextWrapMode::Extend)
                                    .fill(egui::Color32::WHITE);
                            if ui.add(start_over_button).clicked() {
                                self.current_page = None;
                            }
                        });
                        strip.cell(|ui| {
                            if let Some(page) = &self.current_page {
                                let title = egui::Label::new(
                                    egui::RichText::new(page.get_title())
                                        .color(egui::Color32::WHITE)
                                        .heading(),
                                )
                                .wrap_mode(egui::TextWrapMode::Extend);
                                ui.add(title);
                            }
                        });
                        strip.empty();
                    });
            });
    }

    fn show_error_modal(&mut self, ctx: &egui::Context) {
        egui::Modal::new(egui::Id::new("ErrorModal")).show(ctx, |ui| {
            ui.heading("Error");
            ui.label("Sorry, an error has occurred. The install process has been cancelled.");
            ui.separator();
            if let Some(err) = &self.page_error {
                ui.label(format!("{}", err));
            } else {
                ui.label("No error information found.");
            }
            egui_alignments::row(ui, egui::Align::Center, |ui| {
                egui_alignments::stretch(ui);
                if ui.button("Ok").clicked() {
                    self.page_error = None;
                    self.current_page = None;
                }
            });
        });
    }
}

impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        ctx.set_visuals(egui::Visuals::light());
        if self.current_page.is_some() {
            self.add_top_panel(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                if self.page_error.is_some() {
                    self.show_error_modal(ctx);
                } else if let Some(page) = &mut self.current_page {
                    self.page_error = page.run(&mut self.state, ui).err();
                }
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| self.run_start_page(ui));
        }
    }
}
