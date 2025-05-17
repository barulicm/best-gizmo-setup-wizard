use crate::app::GlobalAppState;
use crate::pages::{Page, add_custom_next_button, add_next_button};
use crate::utils::drive_management::{DriveInfo, list_drives};
use crate::utils::github::{GithubRelease, GithubReleaseAsset, download_versioned_asset};
use anyhow::anyhow;
use egui_alignments::{column, stretch};
use std::sync::mpsc::Receiver;

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

    available_relases_receiver: Option<Receiver<Vec<GithubRelease>>>,
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
                    crate::utils::github::get_releases("gizmo-platform", "firmware").unwrap();
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
    }

    fn run_choose_board_revision(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        if self.available_drives.is_none() {
            if let Some(ref version) = self.software_version {
                self.available_firmwares = Some(
                    version
                        .assets
                        .iter()
                        .filter_map(|asset| {
                            let prefix = "gss-";
                            let suffix = "-".to_string() + &version.tag_name + ".uf2";
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
                let version_name = self.software_version.as_ref().unwrap().tag_name.clone();
                let prefix = "gss-";
                let suffix = "-".to_string() + &version_name + ".uf2";
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
    }

    fn run_download_firmware(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        if self.firmware_path.is_none() && self.background_thread.is_none() {
            let release = self.software_version.clone().unwrap();
            let firmware_asset = self.selected_firmware.clone().unwrap();
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
                )
                .unwrap();
                tx.send(download_path).unwrap();
            }));
        }

        if self.download_finished_receiver.is_some() {
            let receiver = self.download_finished_receiver.as_ref().unwrap();
            if let Ok(path) = receiver.try_recv() {
                self.firmware_path = Some(path);
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

        if self.firmware_path.is_some() {
            self.current_step = Step::ChooseDrive;
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Downloading firmware file...");
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
                    .map_err(|e| anyhow!(format!("Failed to join background thread: {:?}", e)))
                    .unwrap();
                self.drive_list_receiver = None;
            }
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
    }

    fn run_install_firmware(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
        if self.install_finished_receiver.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.install_finished_receiver = Some(rx);
            let firmware_path = self.firmware_path.as_ref().unwrap().clone();
            let drive = self.selected_drive.clone().unwrap();
            self.background_thread = Some(std::thread::spawn(move || {
                let filename = firmware_path.file_name().unwrap().to_str().unwrap();
                let destination = drive.drive_path.join(filename);
                std::fs::copy(firmware_path, destination).unwrap();
                tx.send(()).unwrap();
            }));
        }

        if self.install_finished_receiver.is_some() {
            let receiver = self.install_finished_receiver.as_ref().unwrap();
            if let Ok(()) = receiver.try_recv() {
                self.background_thread.take().unwrap().join().unwrap();
                self.install_finished_receiver = None;
                self.current_step = Step::PostInstall;
            }
        }

        column(ui, egui::Align::Center, |ui| {
            stretch(ui);
            ui.spinner();
            ui.label("Installing firmware...");
            stretch(ui);
        });
    }

    fn run_post_install(&mut self, _app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
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
    }
}

impl Page for SystemFirmwarePage {
    fn run(&mut self, app_state: &mut GlobalAppState, ui: &mut egui::Ui) {
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
