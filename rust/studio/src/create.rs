// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! Create functionality

use iced::widget::{column, row, Space};
use iced::{Length, Subscription, Task};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use rfd::FileDialog;
use std::path::PathBuf;

use onerom_config::hw::{Board, MODELS, Model};
use onerom_config::mcu::{Family, MCU_VARIANTS, Variant as McuVariant};
use onerom_fw::net::{Release, Releases};

use crate::app::AppMessage;
use crate::device::{Client, Device, Message as DeviceMessage};
use crate::hw::HardwareInfo;
use crate::studio::{Message as StudioMessage, RuntimeInfo, Images};
use crate::style::Style;
use crate::{task_from_msg, task_from_msgs};
use crate::internal_error;

#[derive(Debug, Clone)]
/// Create tab messages
pub enum Message {
    /// Board selection pick list value changed
    BoardSelected(Board),
    /// Model selection pick list value changed
    ModelSelected(Model),
    /// MCU selection pick list value changed
    McuSelected(McuVariant),
    /// Detect hardware button pressed
    DetectHardware,
    /// Firmware release selected via pick list
    ReleaseSelected(Release),
    /// Releases have been updated (from network)
    ReleasesUpdated,
    /// Hardware information detected from a device or firmware file
    DetectedHardwareInfo,
    /// Config has been selected via pick list
    ConfigSelected(String),
    /// Configs have been updated (from network)
    ConfigsUpdated,
    /// Build images response
    BuildImagesResult(Result<String, String>),
    /// Build images button pressed
    BuildImages,
    /// Save the firmware image
    SaveFirmware,
    /// Save the firmware image with filename
    SaveFirmwareFilename(Option<PathBuf>),
    /// Save firmware operation complete
    SaveFirmwareComplete,
    /// Flash firmware
    FlashFirmware,
    /// Firmware flashing completed
    FlashFirmwareResult(Result<(), String>),
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::BoardSelected(board) => write!(f, "BoardSelected({})", board.name()),
            Message::ModelSelected(model) => write!(f, "ModelSelected({})", model.name()),
            Message::McuSelected(mcu) => write!(f, "McuSelected({mcu})"),
            Message::DetectHardware => write!(f, "DetectHardware"),
            Message::ReleaseSelected(release) => {
                write!(f, "ReleaseSelected({})", release.version)
            }
            Message::ReleasesUpdated => write!(f, "ReleasesUpdated"),
            Message::DetectedHardwareInfo => write!(f, "DetectedHardwareInfo"),
            Message::ConfigSelected(name) => write!(f, "ConfigSelected({})", name),
            Message::ConfigsUpdated => write!(f, "ConfigsUpdated"),
            Message::BuildImagesResult(result) => {
                write!(f, "BuildImagesResult({:?})", result)
            }
            Message::BuildImages => write!(f, "BuildImages"),
            Message::SaveFirmware => write!(f, "SaveFirmware"),
            Message::SaveFirmwareFilename(filename) => {
                write!(f, "SaveFirmwareFilename({:?})", filename)
            }
            Message::SaveFirmwareComplete => write!(f, "SaveFirmwareComplete"),
            Message::FlashFirmware => write!(f, "FlashFirmware"),
            Message::FlashFirmwareResult(result) => {
                write!(f, "FlashFirmwareResult({:?})", result)
            }
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum State {
    #[default]
    Idle,
    Building,
    Flashing,
    Saving,
}

/// Create tab state
#[derive(Debug, Default, Clone)]
pub struct Create {
    selected_hw_info: HardwareInfo,
    mcu_variants: Option<Vec<McuVariant>>,
    display_content: String,
    state: State,
}

impl Create {
    pub const fn top_level_button_name() -> &'static str {
        "Create"
    }

    fn default_display_content() -> String {
        "Image not yet built...".to_string()
    }

    pub fn new() -> Self {
        let mut create = Self::default();
        create.display_content = Self::default_display_content();
        create
    }

    pub fn update(
        &mut self,
        runtime_info: &RuntimeInfo,
        message: Message,
    ) -> iced::Task<AppMessage> {
        match message {
            Message::ModelSelected(model) => {
                debug!("Model selected: {}", model.name());
                self.model_selected(model);
                Task::none()
            }
            Message::BoardSelected(board) => {
                debug!("Board selected: {}", board.name());
                task_from_msg!(self.board_selected(runtime_info, board))
            }
            Message::DetectHardware => {
                info!("Detect hardware requested - unimplemented");
                Task::none()
            }
            Message::McuSelected(mcu) => {
                debug!("MCU selected: {}", mcu);
                self.mcu_selected(mcu);
                task_from_msg!(self.select_latest_release(runtime_info.releases()))
            }
            Message::DetectedHardwareInfo => {
                if let Some(hw_info) = runtime_info.hw_info() {
                    debug!("Incoming detected hardware info: {hw_info}");
                    if let Some(model) = hw_info.model {
                        self.model_selected(model);
                        true
                    } else {
                        false
                    };

                    let msg1 = if self.has_model() && let Some(board) = hw_info.board {
                        self.board_selected(runtime_info, board)
                    } else {
                        None
                    };

                    let msg2 = if self.has_board() && let Some(mcu) = hw_info.mcu_variant {
                        self.mcu_selected(mcu);
                        self.select_latest_release(runtime_info.releases())
                    } else {
                        None
                    };
                    task_from_msgs!([msg1, msg2])
                } else {
                    trace!("No hardware info available");
                    Task::none()
                }
            }
            Message::ReleasesUpdated => {                
                let releases = runtime_info.releases();

                // Select the latest firmware, unless one is already selected
                if self.hardware_selected() && runtime_info.selected_firmware().is_none() {
                    task_from_msg!(self.select_latest_release(releases))
                } else {
                    Task::none()
                }
            }
            Message::ReleaseSelected(release) => {
                debug!("Firmware release selected: {}", release.version);
                task_from_msg!(self.release_selected(release))
            }
            Message::ConfigSelected(name) => {
                debug!("Config selected: {}", name);
                task_from_msg!(self.config_selected(name))
            }
            Message::ConfigsUpdated => {
                // No action needed
                Task::none()
            }
            Message::BuildImages => {
                debug!("Build image requested");
                if self.is_idle() {
                    self.state = State::Building;
                    self.set_display_content("Building image...");
                    Task::done(StudioMessage::BuildImages(self.selected_hw_info.clone()).into())
                } else {
                    warn!("Busy - skipping build images");
                    Task::none()
                }
            }
            Message::BuildImagesResult(result) => {
                debug!("Build image result received: {}", if result.is_ok() { "OK" } else { "Error" });
                if !self.is_busy() {
                    internal_error!("BuildImagesResult received while not busy.");
                }
                if !self.is_building() {
                    internal_error!("BuildImagesResult received while not building.");
                }
                self.state = State::Idle;
                self.build_images_result(result, runtime_info);
                Task::none()
            }
            Message::SaveFirmware => {
                debug!("Save firmware requested");
                if !self.is_busy() {
                    self.state = State::Saving;
                    let filename = format!(
                        "onerom-{}-{}-{}.bin",
                        runtime_info.selected_config().unwrap_or(&"unknown".to_string()),
                        self.selected_hw_info.board_name(),
                        self.selected_hw_info.mcu_name(),
                    );
                    Task::future(Self::save_firmware(filename))
                } else {
                    warn!("Busy - skipping save firmware");
                    return Task::none();
                }
            }
            Message::SaveFirmwareFilename(filename) => {
                debug!("Save firmware filename received: {}", if let Some(f) = &filename { format!("{f:?}") } else { "None".to_string() });
                if self.is_busy() {
                    if filename.is_some() {
                        self.set_display_content(format!("Saving firmware to {filename:?}..."));
                        let images = runtime_info.images().cloned();
                        Task::future(Self::save_firmware_filename(filename, images))
                    } else {
                        debug!("Save firmware cancelled by user");
                        self.state = State::Idle;
                        Task::none()
                    }
                } else {
                    internal_error!("SaveFirmwareFilename received while not saving.");
                    warn!("Aborting save firmware.");
                    Task::none()
                }
            }
            Message::SaveFirmwareComplete => {
                debug!("Save firmware operation complete");
                self.display_content += "\n\nFirmware save complete.";
                if !self.is_busy() {
                    internal_error!("SaveFirmwareComplete received while not saving.");
                }
                if !self.is_saving() {
                    internal_error!("SaveFirmwareComplete received while not saving.");
                }
                self.state = State::Idle;
                Task::none()
            }
            Message::FlashFirmware => {
                debug!("Flash firmware requested");
                if !self.is_busy() {
                    match runtime_info.images().and_then(|imgs| Some(imgs.full_image())) {
                        Some(fw) => {
                            self.state = State::Flashing;
                            self.set_display_content("Flashing firmware...");
                            Task::done((DeviceMessage::FlashFirmware(Client::Create, fw)).into())
                        }
                        None => {
                            self.set_display_content("No firmware image available to flash.");
                            Task::none()
                        }
                    }
                } else {
                    warn!("Busy - skipping flash firmware");
                    return Task::none();
                }

            }
            Message::FlashFirmwareResult(result) => {
                debug!("Flash firmware result received: {}", if result.is_ok() { "OK" } else { "Error" });
                if !self.is_busy() {
                    internal_error!("FlashFirmwareResult received while not busy.");
                }
                if !self.is_flashing() {
                    internal_error!("FlashFirmwareResult received while not flashing.");
                }
                self.state = State::Idle;
                match result {
                    Ok(_) => {
                        self.display_content = "Firmware flashed successfully.".to_string();
                    }
                    Err(e) => {
                        self.display_content = format!("Error flashing firmware:\n  - {e}");
                    }
                }
                Task::none()
            }
        }
    }

    fn set_display_content(&mut self, content: impl ToString) {
        self.display_content = content.to_string();
    }

    fn is_idle(&self) -> bool {
        matches!(self.state, State::Idle)
    }
    fn is_busy(&self) -> bool {
        !self.is_idle()
    }
    fn is_building(&self) -> bool {
        matches!(self.state, State::Building)
    }
    fn is_flashing(&self) -> bool {
        matches!(self.state, State::Flashing)
    }
    fn is_saving(&self) -> bool {
        matches!(self.state, State::Saving)
    }

    async fn save_firmware(filename: String) -> AppMessage {
        let dialog = FileDialog::new()
            .set_title("Save Firmware Image")
            .set_file_name(filename)
            .add_filter("Binary Files", &["bin"])
            .set_directory(".");
        let path = dialog.save_file();
        Message::SaveFirmwareFilename(path).into()
    }

    async fn save_firmware_filename(filename: Option<PathBuf>, images: Option<Images>) -> AppMessage {
        if images.is_none() {
            warn!("No images available to save firmware");
            return Message::SaveFirmwareComplete.into();
        }
        if filename.is_none() {
            debug!("Save firmware cancelled by user");
            return Message::SaveFirmwareComplete.into();
        }
        let images = images.unwrap();
        let filename = filename.unwrap();

        let data = images.full_image();
        match std::fs::write(&filename, data) {
            Ok(_) => {
                debug!("Firmware image saved to {filename:?}");
            }
            Err(e) => {
                error!("Error saving firmware image to {filename:?}: {e}");
            }
        }
        Message::SaveFirmwareComplete.into()
    }

    fn build_images_result(&mut self, result: Result<String, String>, runtime_info: &RuntimeInfo) {
        match result {
            Ok(desc) => {
                self.display_content = format!(
                    "Image built successfully, total: {} bytes ({}/{}/{} plus padding)\n---\n{}",
                    runtime_info.built_full_image_len().unwrap_or(0),
                    runtime_info.built_firmware_len().unwrap_or(0),
                    runtime_info.built_metadata_len().unwrap_or(0),
                    runtime_info.built_roms_len().unwrap_or(0),
                    desc,
                );
            }
            Err(e) => {
                warn!("Error building : {e}");
                self.display_content = format!("Error building image:\n  - {e}");
            }
        }
    }

    fn select_latest_release(&mut self, releases: Option<&Releases>) -> Option<AppMessage> {
        // Only select latest if hardware is fully selected
        if !self.hardware_selected() {
            return None;
        }

        if let Some(releases) = releases {
            let latest = releases.latest();
            let latest = releases.release_from_string(latest);
            if let Some(r) = latest {
                self.release_selected(r.clone())
            } else {
                warn!("No latest release found in releases");
                None
            }
        } else {
            warn!("Release updated but no releases");
            None
        }
    }

    fn release_selected(&mut self, release: Release) -> Option<AppMessage> {
        // Download the release
        if let Some(board) = self.selected_hw_info.board
            && let Some(mcu) = self.selected_hw_info.mcu_variant
        {
            Some(AppMessage::Studio(StudioMessage::DownloadRelease(
                release, board, mcu,
            )))
        } else {
            warn!("Board or MCU not selected, cannot download firmware");
            None
        }
    }

    fn config_selected(&mut self, name: String) -> Option<AppMessage> {
        self.display_content = Self::default_display_content();
        Some(AppMessage::Studio(StudioMessage::DownloadConfig(name)))
    }

    fn has_model(&self) -> bool {
        self.selected_hw_info.model.is_some()
    }
    fn has_board(&self) -> bool {
        self.selected_hw_info.board.is_some()
    }
    #[allow(dead_code)]
    fn has_mcu(&self) -> bool {
        self.selected_hw_info.mcu_variant.is_some()
    }

    fn model_selected(&mut self, model: Model) {
        self.selected_hw_info.model = Some(model);
        self.selected_hw_info.board = None;
        self.selected_hw_info.mcu_variant = None;
        self.mcu_variants = None;
    }

    fn board_selected(&mut self, runtime_info: &RuntimeInfo, board: Board) -> Option<AppMessage> {
        self.selected_hw_info.board = Some(board);
        let mut vars = Vec::new();
        for var in MCU_VARIANTS {
            if board.mcu_family() == var.family() {
                vars.push(*var);
            }
        }
        self.mcu_variants = Some(vars);

        // Special case the Fire boards
        if board.mcu_family() == Family::Rp2350 {
            self.mcu_selected(McuVariant::RP2350);
            self.select_latest_release(runtime_info.releases())
        } else {
            Some(self.clear_mcu())
        }
    }

    fn mcu_selected(&mut self, mcu: McuVariant) {
        self.selected_hw_info.mcu_variant = Some(mcu);
    }

    fn clear_mcu(&mut self) -> AppMessage {
        self.selected_hw_info.mcu_variant = None;
        StudioMessage::ClearDownloadedRelease.into()
    }

    fn hardware_selected(&self) -> bool {
        self.selected_hw_info.is_complete()
    }

    fn ready_to_build(&self, runtime_info: &RuntimeInfo) -> bool {
        self.hardware_selected() && runtime_info.selected_firmware().is_some() && runtime_info.config().is_some()
    }

    pub fn view<'a>(&'a self, runtime_info: &'a RuntimeInfo, device: &Device) -> iced::Element<'a, AppMessage> {
        let mut columns = column![
            row![
                self.select_hw_heading_row(),
                //Style::text_h3("or"),
                //self.detect_button(),
            ]
            .spacing(20)
            .align_y(iced::alignment::Vertical::Center),
            self.select_hw_row(),
            self.board_description_row(),
            Style::horiz_line()
        ];

        if self.hardware_selected() {
            // Add row to column
            columns = columns
                .push(self.firmware_row(runtime_info))
                .push(Style::horiz_line());
        }

        if self.hardware_selected() && runtime_info.configs().is_some() {
            // Add row to column
            columns = columns
                .push(self.config_row(runtime_info))
                .push(Style::horiz_line());
        }

        if self.ready_to_build(runtime_info) {
            let content = if self.is_building() {
                "Building...".to_string()
            } else {
                "Build Image".to_string()
            };
            let (on_press, highlighted) = if self.is_busy() {
                (None, false)
            } else {
                (Some(Message::BuildImages.into()), true)
            };
            let build_button = Style::text_button_small(
                content,
                on_press,
                highlighted,
            );

            let button_row = row![build_button].spacing(20);

            // Only show Save and Flash buttons if images exist _and_ not in
            // the process of building one.
            let button_row = if runtime_info.images().is_some() && !self.is_building() {
                let (on_press, highlighted) = if self.is_busy() {
                    (None, false)
                } else {
                    (Some(Message::SaveFirmware.into()), true)
                };
                let save_content = if self.is_saving() {
                    "Saving...".to_string()
                } else {
                    "Save Firmware".to_string()
                };
                let save_button = Style::text_button_small(
                    save_content,
                    on_press,
                    highlighted,
                );

                let flash_content = if self.is_flashing() {
                    "Flashing...".to_string()
                } else {
                    "Flash Firmware".to_string()
                };
                let (on_press, highlighted) = if self.is_busy() || device.selected().is_none() {
                    (None, false)
                } else {
                    (Some(Message::FlashFirmware.into()), true)
                };
                let flash_button = Style::text_button_small(
                    flash_content,
                    on_press,
                    highlighted,
                );

                button_row
                    .push(Space::with_width(Length::Fill))
                    .push(save_button)
                    .push(flash_button)
            } else {
                button_row
            };

            let window = Style::box_scrollable_text(
                self.display_content.clone(),
                166.0,
                true,
            );
            let window_container = Style::container(window);

            columns = columns.push(button_row);
            columns = columns.push(window_container);
        }

        columns.spacing(20).into()
    }

    fn config_row<'a>(&'a self, runtime_info: &'a RuntimeInfo) -> iced::Element<'a, AppMessage> {
        // Create config selection row
        if let Some(configs) = &runtime_info.configs() {
            let selected_config = runtime_info.selected_config();

            let config_names = configs.names();

            let pick_list = Style::pick_list_small(
                config_names.as_slice(),
                selected_config,
                |name| AppMessage::Create(Message::ConfigSelected(name)),
            );

            let mut row = row![
                Style::text_h3("ROM Config:"),
                pick_list,
            ];

            if selected_config.is_some() {
                // Show if config has been downloaded
                if let Some(config_len) = runtime_info.config_len() {
                    // split into three rows, with number of bytes gold
                    let downloaded_row = row![
                        Style::text_small("(downloaded: "),
                        Style::text_small(format!("{}", config_len)).color(Style::COLOUR_DARK_GOLD),
                        Style::text_small(" bytes)"),
                    ]
                    .spacing(0);
                    row = row.push(downloaded_row);
                }
            }

            row.spacing(20)
                .align_y(iced::alignment::Vertical::Center)
                .into()
        } else {
            row![Style::text_h3("No configurations available")]
                .spacing(20)
                .align_y(iced::alignment::Vertical::Center)
                .into()
        }
    }

    fn firmware_row<'a>(&'a self, runtime_info: &'a RuntimeInfo) -> iced::Element<'a, AppMessage> {
        // Create release selection row
        if let Some(releases) = &runtime_info.releases() {
            let latest = releases.latest();

            let selected_release = if let Some(r) = runtime_info.selected_firmware() {
                Some(r)
            } else {
                releases.release_from_string(latest)
            };

            let mut rows = row![
                Style::text_h3("Firmware Release"),
                Style::pick_list_small(releases.releases().as_slice(), selected_release, |r| {
                    AppMessage::Create(Message::ReleaseSelected(r))
                })
            ];

            // Show if release has been downloaded
            if let Some(fw_len) = runtime_info.firmware_len() {
                // split into three rows, with number of bytes gold
                let downloaded_row = row![
                    Style::text_small("(downloaded: "),
                    Style::text_small(format!("{}", fw_len)).color(Style::COLOUR_DARK_GOLD),
                    Style::text_small(" bytes)"),
                ]
                .spacing(0);
                rows = rows.push(downloaded_row);
            }

            // Return the row
            rows.spacing(20).align_y(iced::alignment::Vertical::Center)
        } else {
            row![Style::text_h3("No firmware releases available")]
        }
        .spacing(20)
        .align_y(iced::alignment::Vertical::Center)
        .into()
    }

    fn select_hw_heading_row(&self) -> iced::Element<'_, AppMessage> {
        row![Style::text_h3("Select Hardware")].into()
    }

    //fn detect_button(&self) -> iced::Element<'_, AppMessage> {
    //    let button = Style::text_button_small(
    //        "Detect Hardware",
    //        Some(Message::DetectHardware.into()),
    //        true,
    //    );
    //    row![button].into()
    //}

    fn select_hw_row(&self) -> iced::Element<'_, AppMessage> {
        // Set up model picker
        let model_picker =
            Style::pick_list_small(MODELS.as_slice(), self.selected_hw_info.model, |model| {
                AppMessage::Create(Message::ModelSelected(model))
            });
        let model_picker = row![Style::text_body("Model:"), model_picker,]
            .spacing(10)
            .align_y(iced::alignment::Vertical::Center);

        // Set up board picker
        let board_values = if let Some(model) = self.selected_hw_info.model {
            model.boards()
        } else {
            &[]
        };
        let board_picker =
            Style::pick_list_small(board_values, self.selected_hw_info.board, |board| {
                AppMessage::Create(Message::BoardSelected(board))
            });
        let board_picker = row![Style::text_body("Board:"), board_picker,]
            .spacing(10)
            .align_y(iced::alignment::Vertical::Center);

        // Set up MCU picker
        let mcu_values = if let Some(vars) = &self.mcu_variants {
            vars.as_slice()
        } else {
            &[]
        };
        let mcu_picker =
            Style::pick_list_small(mcu_values, self.selected_hw_info.mcu_variant, |mcu| {
                AppMessage::Create(Message::McuSelected(mcu))
            });
        let mcu_picker = row![Style::text_body("MCU:"), mcu_picker,]
            .spacing(10)
            .align_y(iced::alignment::Vertical::Center);

        row![model_picker, board_picker, mcu_picker]
            .spacing(20)
            .into()
    }

    fn board_description_row(&self) -> iced::Element<'_, AppMessage> {
        if self.hardware_selected() {
            let hw_info_row = Style::hw_info_row(
                None,
                None,
                self.selected_hw_info.model,
                self.selected_hw_info.board,
                self.selected_hw_info.mcu_variant,
                true,
            );

            row![
                Style::text_body("Selected:").color(Style::COLOUR_GOLD),
                hw_info_row,
            ]
            .spacing(20)
            .align_y(iced::alignment::Vertical::Center)
        } else {
            row![Style::text_body("Hardware not selected")]
        }
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}
