use crate::app::GlobalAppState;
use crate::pages::{Page, add_custom_next_button, add_next_button};
use crate::utils::drive_management::{DriveInfo, list_drives};
use crate::utils::github::GithubRelease;
use crate::utils::threads::join_thread;
use anyhow::anyhow;
use egui_alignments::{column, stretch};
use std::sync::mpsc::Receiver;
use std::time::Duration;

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

    available_releases_receiver: Option<Receiver<Vec<GithubRelease>>>,
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

            available_releases_receiver: None,
            download_finished_receiver: None,
            drive_list_receiver: None,
            install_finished_receiver: None,

            background_thread: None,
        }
    }

    fn run_choose_version(
        &mut self,
        _app_state: &mut GlobalAppState,
        ui: &mut egui::Ui,
    ) -> anyhow::Result<()> {
        if self.available_releases.is_none() && self.background_thread.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.available_releases_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let releases = crate::utils::github::get_releases("gizmo-platform", "gizmo")
                    .expect("Failed to fetch GitHub releases.");
                tx.send(releases)
                    .expect("Failed to send release details to main thread.");
            }));
        }
        if let Some(thread) = self.background_thread.take_if(|t| t.is_finished()) {
            join_thread(thread)?;
            let receiver = self.available_releases_receiver.take().ok_or(anyhow!(
                "Expected available_releases_receiver to not be None."
            ))?;
            self.available_releases = Some(receiver.recv_timeout(Duration::from_secs(1))?);
        }
        if self.available_releases.is_some() && self.software_version.is_none() {
            self.software_version = Some(
                self.available_releases
                    .as_ref()
                    .ok_or(anyhow!("Expected available_releases to not be None."))?
                    .iter()
                    .find(|r| r.latest)
                    .ok_or(anyhow!("Latest release not found"))?
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
        Ok(())
    }

    fn run_enter_team_numbers(
        &mut self,
        _app_state: &mut GlobalAppState,
        ui: &mut egui::Ui,
    ) -> anyhow::Result<()> {
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
        Ok(())
    }

    fn run_download_archive(
        &mut self,
        app_state: &mut GlobalAppState,
        ui: &mut egui::Ui,
    ) -> anyhow::Result<()> {
        if self.archive_path.is_none() && self.background_thread.is_none() {
            let thread_release = self
                .software_version
                .clone()
                .ok_or(anyhow!("Expected software_version to not be None."))?;
            let cache_path = app_state.tmp_dir.path().join("github_downloads");
            let (tx, rx) = std::sync::mpsc::channel();
            self.download_finished_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let asset = thread_release
                    .assets
                    .iter()
                    .find(|a| a.name == "ds-ramdisk.zip")
                    .expect("Could not find ds-ramdisk.zip in release assets.");
                let archive_path = crate::utils::github::download_versioned_asset(
                    asset,
                    "gizmo-platform",
                    "gizmo",
                    &thread_release,
                    &cache_path,
                )
                .expect("Failed to download ramdisk archive.");
                tx.send(archive_path)
                    .expect("Failed to send download path to main thread.");
            }));
        }

        if let Some(thread) = self.background_thread.take_if(|t| t.is_finished()) {
            join_thread(thread)?;
            let receiver = self.download_finished_receiver.take().ok_or(anyhow!(
                "Expected download_finished_receiver to not be None."
            ))?;
            self.archive_path = Some(receiver.recv_timeout(Duration::from_secs(1))?);
            self.current_step = Step::ChooseDrive;
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Downloading software archive...");
            stretch(ui);
        });
        Ok(())
    }

    fn run_choose_drive(
        &mut self,
        _app_state: &mut GlobalAppState,
        ui: &mut egui::Ui,
    ) -> anyhow::Result<()> {
        if self.available_drives.is_none() && self.background_thread.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.drive_list_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let drives = list_drives().expect("Falied to get list of available drives.");
                tx.send(drives)
                    .expect("Failed to send drive list to main thread.");
            }));
        }

        if let Some(thread) = self.background_thread.take_if(|t| t.is_finished()) {
            join_thread(thread)?;
            let receiver = self
                .drive_list_receiver
                .take()
                .ok_or(anyhow!("Expected drive_list_receiver to not be None."))?;
            self.available_drives = Some(receiver.recv_timeout(Duration::from_secs(1))?);
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
        Ok(())
    }

    fn run_install_software(
        &mut self,
        _app_state: &mut GlobalAppState,
        ui: &mut egui::Ui,
    ) -> anyhow::Result<()> {
        if self.install_finished_receiver.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.install_finished_receiver = Some(rx);
            let archive_path = self
                .archive_path
                .as_ref()
                .ok_or(anyhow!("Expected archive_path to not be None."))?;
            #[allow(unused_mut)] // drive needs to be mutable on Linux, but not on Windows
            let mut drive = self
                .selected_drive
                .clone()
                .ok_or(anyhow!("Expected selected_drive to not be None."))?;
            let ramdisk_archive = std::fs::File::open(archive_path)?;
            let team_number = self.team_numbers[self.team_number_index].clone();
            self.background_thread = Some(std::thread::spawn(move || {
                crate::utils::drive_management::format_drive(&drive, &team_number)
                    .expect("Failed to format drive.");
                #[cfg(target_os = "linux")]
                {
                    // On linux, the drive path includes the volume label, so we need to update the
                    // path after we change the name during formatting.
                    drive.drive_path = drive
                        .drive_path
                        .parent()
                        .expect("Failed to get parent path of drive path")
                        .join(format!("GIZMO{team_number}"));
                };
                zip_extract::extract(ramdisk_archive, &drive.drive_path, true)
                    .expect("Failed to extract ramdisk archive.");
                crate::utils::drive_management::write_filesystem_cache(&drive)
                    .expect("Failed to flush filesystem cache.");
                tx.send(())
                    .expect("Failed to signal intall finish to main thread.");
            }));
        }

        if let Some(thread) = self.background_thread.take_if(|t| t.is_finished()) {
            join_thread(thread)?;
            self.install_finished_receiver.take().ok_or(anyhow!(
                "Expected install_finished_receiver to not be None."
            ))?;
            self.current_step = Step::RemoveCard;
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Installing software...");
            stretch(ui);
        });
        Ok(())
    }

    fn run_remove_card(
        &mut self,
        _app_state: &mut GlobalAppState,
        ui: &mut egui::Ui,
    ) -> anyhow::Result<()> {
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
        Ok(())
    }
}

impl Page for DriverStationSetupPage {
    fn run(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
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
