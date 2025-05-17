use crate::app::GlobalAppState;
use crate::pages::{Page, add_custom_next_button, add_next_button};
use crate::utils::drive_management::{DriveInfo, list_drives};
use crate::utils::github::{GithubRelease, GithubReleaseAsset, download_versioned_asset};
use crate::utils::threads::join_thread;
use anyhow::anyhow;
use egui_alignments::{column, stretch};
use std::sync::mpsc::Receiver;
use std::time::Duration;

enum Step {
    ChooseVersion,
    ChooseBoardRevision,
    DownloadFirmware,
    ChooseDrive,
    InstallFirmware,
    PostInstall,
}

pub struct SystemFirmwarePage {
    current_step: Step,
    available_releases: Option<Vec<GithubRelease>>,
    software_version: Option<GithubRelease>,
    available_firmwares: Option<Vec<GithubReleaseAsset>>,
    selected_firmware: Option<GithubReleaseAsset>,
    firmware_path: Option<std::path::PathBuf>,
    available_drives: Option<Vec<DriveInfo>>,
    selected_drive: Option<DriveInfo>,

    available_releases_receiver: Option<Receiver<Vec<GithubRelease>>>,
    download_finished_receiver: Option<Receiver<std::path::PathBuf>>,
    drive_list_receiver: Option<Receiver<Vec<DriveInfo>>>,
    install_finished_receiver: Option<Receiver<()>>,

    background_thread: Option<std::thread::JoinHandle<()>>,
}

impl SystemFirmwarePage {
    pub fn new() -> Self {
        Self {
            current_step: Step::ChooseVersion,
            available_releases: None,
            software_version: None,
            available_firmwares: None,
            selected_firmware: None,
            firmware_path: None,
            available_drives: None,
            selected_drive: None,

            available_releases_receiver: None,
            download_finished_receiver: None,
            drive_list_receiver: None,
            install_finished_receiver: None,

            background_thread: None,
        }
    }

    fn run_choose_version(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
        if self.available_releases.is_none() && self.background_thread.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.available_releases_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let releases =
                    crate::utils::github::get_releases("gizmo-platform", "firmware").expect("Failed to fetch GitHub releases.");
                tx.send(releases).expect("Failed to send release details to main thread.");
            }));
        }
        if let Some(thread) = self.background_thread.take_if(|t| { t.is_finished() }) {
            join_thread(thread)?;
            let receiver = self.available_releases_receiver.take().ok_or(anyhow!("Expected available_releases_receiver to not be None."))?;
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
            ui.heading("Firmware Version");
            ui.label("Select the version of the firmware you want to install. Usually, this should be the latest version.");
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
                self.current_step = Step::ChooseBoardRevision;
            }
        });
        Ok(())
    }

    fn run_choose_board_revision(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
        let version_name = self.software_version.as_ref().ok_or(anyhow!("Expected software_version to not be None."))?.tag_name.clone();
        let prefix = "gss-";
        let suffix = "-".to_string() + &version_name + ".uf2";

        if self.available_firmwares.is_none() {
            if let Some(ref version) = self.software_version {
                self.available_firmwares = Some(
                    version
                        .assets
                        .iter()
                        .filter_map(|asset| {
                            if asset.name.starts_with(&prefix) && asset.name.ends_with(&suffix) {
                                Some(asset.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                );
            }
        }

        column(ui, egui::Align::LEFT, |ui| {
            ui.heading("Choose Hardware Version");
            ui.label("Select the hardware version of the Gizmo PCB you are using. This should be printed on the board and should look something like \"v01.00\" or \"v00.r6b\"");

            if let Some(ref available_revisions) = self.available_firmwares {
                for rev in available_revisions {
                    let display_text = rev
                        .name
                        .trim_start_matches(&prefix)
                        .trim_end_matches(&suffix);
                    ui.selectable_value(
                        &mut self.selected_firmware,
                        Some(rev.clone()),
                        display_text,
                    );
                }
            } else {
                ui.colored_label(
                    egui::Color32::DARK_RED,
                    "Could not recognize any firmware files in the selected release.",
                );
            }

            stretch(ui);
            if add_next_button(ui, self.selected_firmware.is_some()).clicked() {
                self.current_step = Step::DownloadFirmware;
            }
        });
        Ok(())
    }

    fn run_download_firmware(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
        if self.firmware_path.is_none() && self.background_thread.is_none() {
            let release = self.software_version.clone().ok_or(anyhow!("Expected software_version to not be None."))?;
            let firmware_asset = self.selected_firmware.clone().ok_or(anyhow!("Expected selected_firmware to not be None."))?;
            let cache_path = app_state.tmp_dir.path().join("github_downloads");
            let (tx, rx) = std::sync::mpsc::channel();
            self.download_finished_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let download_path = download_versioned_asset(
                    &firmware_asset,
                    "gizmo-platform",
                    "firmware",
                    &release,
                    &cache_path,
                ).expect("Falied to fetch GitHub releases.");
                tx.send(download_path).expect("Failed to send release details to main thread.");
            }));
        }

        if let Some(thread) = self.background_thread.take_if(|t| { t.is_finished() }) {
            join_thread(thread)?;
            let receiver = self.download_finished_receiver.take().ok_or(anyhow!("Expected download_finished_receiver to not be None."))?;
            self.firmware_path = Some(receiver.recv_timeout(Duration::from_secs(1))?);
            self.current_step = Step::ChooseDrive;
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Downloading firmware file...");
            stretch(ui);
        });
        Ok(())
    }

    fn run_choose_drive(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
        if self.available_drives.is_none() && self.background_thread.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.drive_list_receiver = Some(rx);
            self.background_thread = Some(std::thread::spawn(move || {
                let drives = list_drives().expect("Failed to get list of available drives.");
                tx.send(drives).expect("Failed to send drive list to main thread.");
            }));
        }

        if let Some(thread) = self.background_thread.take_if(|t| { t.is_finished() }) {
            join_thread(thread)?;
            let receiver = self.drive_list_receiver.take().ok_or(anyhow!("Expected drive_list_receiver to not be None."))?;
            self.available_drives = Some(receiver.recv_timeout(Duration::from_secs(1))?);
        }

        column(ui, egui::Align::LEFT, |ui| {
            ui.heading("Choose Device");
            ui.label(
                r#"1. Press and hold the BOOTSEL button on the system processor.
2. Connect the system processor to your computer with the USB cable.
3. Release the BOOTSEL button.
4. Click the "Refresh" button to update the list below.
5. Select the drive from the list and click "Install Firmware". The drive should be named "RPI-RP2".
"#,
            );
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
            if add_custom_next_button(ui, "Install Firmware", self.selected_drive.is_some())
                .clicked()
            {
                self.current_step = Step::InstallFirmware;
            }
        });
        Ok(())
    }

    fn run_install_firmware(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
        if self.install_finished_receiver.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.install_finished_receiver = Some(rx);
            let firmware_path = self.firmware_path.clone().ok_or(anyhow!("Expected firmware_path to not be None."))?;
            let drive = self.selected_drive.clone().ok_or(anyhow!("Expected selected_drive to not be None."))?;
            let filename = firmware_path.file_name().ok_or(anyhow!("Could not find filename in firmware_path"))?.to_str().ok_or(anyhow!("Could not convert filename to string."))?;
            let destination = drive.drive_path.join(filename);
            self.background_thread = Some(std::thread::spawn(move || {
                std::fs::copy(firmware_path, destination).expect("Failed to copy firmware to device.");
                tx.send(()).expect("Failed to signal install finish to main thread.");
            }));
        }

        if let Some(thread) = self.background_thread.take_if(|t| { t.is_finished() }) {
            join_thread(thread)?;
            self.install_finished_receiver.take().ok_or(anyhow!("Expected install_finished_receiver to not be None."))?;
            self.current_step = Step::PostInstall;
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Installing firmware...");
            stretch(ui);
        });
        Ok(())
    }

    fn run_post_install(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
        column(ui, egui::Align::LEFT, |ui| {
            ui.heading("Installation Complete");
            ui.label("You can now disconnect the device from the computer.");
            ui.label("To install system firmware onto another device, click \"Setup Another Device\". If you are done installing system firmware, you can close the wizard or click \"Start Over\".");
            stretch(ui);
            if add_custom_next_button(ui, "Setup Another Device", true).clicked() {
                self.selected_drive = None;
                self.available_drives = None;
                self.current_step = Step::ChooseDrive
            }
        });
        Ok(())
    }
}

impl Page for SystemFirmwarePage {
    fn run(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) -> anyhow::Result<()> {
        match self.current_step {
            Step::ChooseVersion => self.run_choose_version(app_state, ui),
            Step::ChooseBoardRevision => self.run_choose_board_revision(app_state, ui),
            Step::DownloadFirmware => self.run_download_firmware(app_state, ui),
            Step::ChooseDrive => self.run_choose_drive(app_state, ui),
            Step::InstallFirmware => self.run_install_firmware(app_state, ui),
            Step::PostInstall => self.run_post_install(app_state, ui),
        }
    }

    fn get_title(&self) -> String {
        "System Firmware Install".to_string()
    }
}
