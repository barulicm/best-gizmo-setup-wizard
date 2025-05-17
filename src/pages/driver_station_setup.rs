use crate::app::GlobalAppState;
use crate::pages::{Page, add_custom_next_button, add_next_button};
use crate::utils::drive_management::{DriveInfo, list_drives};
use crate::utils::github::GithubRelease;
use anyhow::anyhow;
use egui_alignments::{column, stretch};
use std::sync::mpsc::Receiver;

enum Step {
    ChooseVersion,
    EnterTeamNumbers,
    DownloadArchive,
    ChooseDrive,
    InstallSoftware,
    RemoveCard,
}

pub struct DriverStationSetupPage {
    current_step: Step,
    available_releases: Option<Vec<GithubRelease>>,
    software_version: Option<GithubRelease>,
    archive_path: Option<std::path::PathBuf>,
    team_numbers_text: String,
    team_numbers: Vec<String>,
    team_number_index: usize,
    available_drives: Option<Vec<DriveInfo>>,
    selected_drive: Option<DriveInfo>,

    available_relases_receiver: Option<Receiver<Vec<GithubRelease>>>,
    download_finished_receiver: Option<Receiver<std::path::PathBuf>>,
    drive_list_receiver: Option<Receiver<Vec<DriveInfo>>>,
    install_finished_receiver: Option<Receiver<()>>,

    background_thread: Option<std::thread::JoinHandle<()>>,
}

impl DriverStationSetupPage {
    pub fn new() -> Self {
        Self {
            current_step: Step::ChooseVersion,
            available_releases: None,
            software_version: None,
            archive_path: None,
            team_numbers_text: String::new(),
            team_numbers: vec![],
            team_number_index: 0,
            available_drives: None,
            selected_drive: None,

            available_relases_receiver: None,
            download_finished_receiver: None,
            drive_list_receiver: None,
            install_finished_receiver: None,

            background_thread: None,
        }
    }

    fn run_choose_version(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        if self.available_releases.is_none() && self.background_thread.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.available_relases_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let releases =
                    crate::utils::github::get_releases("gizmo-platform", "gizmo").unwrap();
                tx.send(releases).unwrap();
            }));
        }
        if let Some(ref receiver) = self.available_relases_receiver {
            if let Ok(releases) = receiver.try_recv() {
                self.available_releases = Some(releases);
                let thread = self.background_thread.take().unwrap();
                thread
                    .join()
                    .map_err(|e| {
                        anyhow::Error::msg(format!("Failed to join background thread: {:?}", e))
                    })
                    .unwrap();
            }
        }
        if self.background_thread.is_none() && self.available_releases.is_some() {
            self.available_relases_receiver = None;
        }
        if self.available_releases.is_some() && self.software_version.is_none() {
            self.software_version = Some(
                self.available_releases
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|r| r.latest)
                    .ok_or(anyhow!("Latest release not found"))
                    .unwrap()
                    .clone(),
            );
        }
        let next_button_enabled = self.software_version.is_some();

        column(ui, egui::Align::LEFT, |ui| {
            ui.heading("Software Version");
            ui.label("Select the version of the software you want to install. Usually, this should be the latest version.");
            if let Some(ref releases) = self.available_releases {
                egui::ComboBox::from_label("Pick a version")
                    .selected_text(match self.software_version {
                        Some(ref version) => version.display_name(),
                        None => "Select Version".to_string(),
                    })
                    .show_ui(ui, |ui| {
                        for release in releases {
                            ui.selectable_value(
                                &mut self.software_version,
                                Some(release.clone()),
                                release.display_name(),
                            );
                        }
                    });
            } else {
                ui.spinner();
                ui.label("Fetching available releases...");
            }
            stretch(ui);
            if add_next_button(ui, next_button_enabled).clicked() {
                self.current_step = Step::EnterTeamNumbers;
            }
        });
    }

    fn run_enter_team_numbers(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        column(ui, egui::Align::LEFT, |ui| {
            ui.heading("Team Numbers");
            ui.label("Enter your team numbers, one per line.");

            let text_edit_response = ui.text_edit_multiline(&mut self.team_numbers_text);
            let text_valid = self
                .team_numbers_text
                .chars()
                .all(|c| c.is_ascii_digit() || c == '\n');
            if !text_valid {
                ui.colored_label(egui::Color32::DARK_RED, "Invalid team numbers.");
                self.team_numbers.clear();
            }
            if text_valid && text_edit_response.changed() {
                self.team_numbers = self
                    .team_numbers_text
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
            ui.label(format!("{} team numbers.", self.team_numbers.len()));

            stretch(ui);

            if add_next_button(ui, !self.team_numbers.is_empty()).clicked() {
                self.current_step = Step::DownloadArchive;
            }
        });
    }

    fn run_download_archive(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        if self.archive_path.is_none() && self.background_thread.is_none() {
            let thread_release = self.software_version.clone().unwrap();
            let cache_path = app_state.tmp_dir.path().join("github_downloads");
            let (tx, rx) = std::sync::mpsc::channel();
            self.download_finished_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let asset = thread_release
                    .assets
                    .iter()
                    .find(|a| a.name == "ds-ramdisk.zip")
                    .ok_or(anyhow!("Could not find ds-ramdisk.zip in release assets."))
                    .unwrap();
                let archive_path = crate::utils::github::download_versioned_asset(
                    &asset,
                    "gizmo-platform",
                    "gizmo",
                    &thread_release,
                    &cache_path,
                )
                .unwrap();
                tx.send(archive_path).unwrap();
            }));
        }

        if self.download_finished_receiver.is_some() {
            let receiver = self.download_finished_receiver.as_ref().unwrap();
            if let Ok(path) = receiver.try_recv() {
                self.archive_path = Some(path);
                let thread = self.background_thread.take().unwrap();
                thread
                    .join()
                    .map_err(|e| {
                        anyhow::Error::msg(format!("Failed to join background thread: {:?}", e))
                    })
                    .unwrap();
                self.download_finished_receiver = None;
            }
        }

        if self.archive_path.is_some() {
            self.current_step = Step::ChooseDrive;
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Downloading software archive...");
            stretch(ui);
        });
    }

    fn run_choose_drive(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        if self.available_drives.is_none() && self.background_thread.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.drive_list_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let drives = list_drives().unwrap();
                tx.send(drives).unwrap();
            }));
        }

        if self.drive_list_receiver.is_some() {
            let receiver = self.drive_list_receiver.as_ref().unwrap();
            if let Ok(drives) = receiver.try_recv() {
                self.available_drives = Some(drives);
                let thread = self.background_thread.take().unwrap();
                thread
                    .join()
                    .map_err(|e| {
                        anyhow::Error::msg(format!("Failed to join background thread: {:?}", e))
                    })
                    .unwrap();
                self.drive_list_receiver = None;
            }
        }

        column(ui, egui::Align::LEFT, |ui| {
            ui.heading("Choose Drive");

            let team_number = self.team_numbers[self.team_number_index].clone();
            ui.label(format!(
                r#"Setting up driver station for team {team_number}.
            
1. Insert the microSD card for this team into your computer.
2. Click the "Refresh" button to update the list below.
3. Select the microSD card drive from the list and click "Install Software".
"#
            ));

            if let Some(ref drives) = self.available_drives {
                if drives.is_empty() {
                    ui.label("No removable drives found.");
                } else {
                    for drive in drives {
                        ui.selectable_value(
                            &mut self.selected_drive,
                            Some(drive.clone()),
                            format!("{drive}"),
                        );
                    }
                }

                if ui.button("Refresh").clicked() {
                    self.available_drives = None;
                    self.selected_drive = None;
                }
            } else {
                ui.spinner();
                ui.label("Searching for removable drives...");
            }

            stretch(ui);

            if add_custom_next_button(ui, "Install Software", self.selected_drive.is_some())
                .clicked()
            {
                self.current_step = Step::InstallSoftware;
            }
        });
    }

    fn run_install_software(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        if self.install_finished_receiver.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.install_finished_receiver = Some(rx);
            let archive_path = self.archive_path.as_ref().unwrap();
            let drive = self.selected_drive.as_ref().unwrap().clone();
            let ramdisk_archive = std::fs::File::open(&archive_path).unwrap();
            let team_number = self.team_numbers[self.team_number_index].clone();
            self.background_thread = Some(std::thread::spawn(move || {
                crate::utils::drive_management::format_drive(&drive, &team_number).unwrap();
                zip_extract::extract(ramdisk_archive, &drive.drive_path, true).unwrap();
                crate::utils::drive_management::write_filesystem_cache(&drive).unwrap();
                tx.send(()).unwrap();
            }));
        }

        if self.install_finished_receiver.is_some() {
            let receiver = self.install_finished_receiver.as_ref().unwrap();
            if let Ok(()) = receiver.try_recv() {
                self.background_thread.take().unwrap().join().unwrap();
                self.install_finished_receiver = None;
                self.current_step = Step::RemoveCard;
            }
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Installing software...");
            stretch(ui);
        });
    }

    fn run_remove_card(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        column(ui, egui::Align::LEFT, |ui| {
            ui.heading("Installation Complete");
            let team_number = self.team_numbers[self.team_number_index].clone();
            ui.label(format!("Please remove the card from the drive and insert it into the driver station for team {team_number}."));

            if self.team_number_index < self.team_numbers.len() - 1 {
                ui.label("Once you have done this, click Next.");
                stretch(ui);
                if add_next_button(ui, true).clicked() {
                    self.team_number_index += 1;
                    self.selected_drive = None;
                    self.available_drives = None;
                    self.current_step = Step::ChooseDrive;
                }
            } else {
                ui.label("All team numbers have been processed. You can now close the wizard or click 'Start Over'.");
            }
        });
    }
}

impl Page for DriverStationSetupPage {
    fn run(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        match self.current_step {
            Step::ChooseVersion => self.run_choose_version(app_state, ui),
            Step::EnterTeamNumbers => self.run_enter_team_numbers(app_state, ui),
            Step::DownloadArchive => self.run_download_archive(app_state, ui),
            Step::ChooseDrive => self.run_choose_drive(app_state, ui),
            Step::InstallSoftware => self.run_install_software(app_state, ui),
            Step::RemoveCard => self.run_remove_card(app_state, ui),
        }
    }

    fn get_title(&self) -> String {
        "Driver Station Software Install".to_string()
    }
}
