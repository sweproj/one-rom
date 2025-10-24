// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! Analyse image functionality

use iced::widget::{Button, Space, column, row};
use iced::{Element, Length, Subscription, Task};
use rfd::FileDialog;
use std::path::PathBuf;

#[allow(unused_imports)]
use onerom_config::fw::FirmwareVersion;
use onerom_config::mcu::Variant as McuVariant;
use sdrr_fw_parser::{Parser, SdrrInfo, readers::MemoryReader};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use crate::app::AppMessage;
use crate::device::{Device, Message as DeviceMessage, Client};
use crate::hw::HardwareInfo;
use crate::studio::{Message as StudioMessage, RuntimeInfo};
use crate::style::Style;

const FW_VERSION_0_5_0: FirmwareVersion = FirmwareVersion::new(0, 5, 0, 0);

/// Analyse tab messages
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    SourceTabSelected(SourceTab),
    DetectDevice,
    SelectFile,
    FileSelected(Option<PathBuf>),
    FileLoaded(Result<(SdrrInfo, Vec<u8>), String>),
    DeviceLoaded(Result<(SdrrInfo, Vec<u8>), String>),
    DeviceData(Vec<u8>),
    ReadFailed(String),
    RereadDevice(McuVariant, FirmwareVersion),
    FlashFirmware,
    FlashComplete(Result<(), String>),
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::SourceTabSelected(tab) => write!(f, "SourceTabSelected({:?})", tab),
            Message::DetectDevice => write!(f, "DetectDevice"),
            Message::SelectFile => write!(f, "SelectFile"),
            Message::FileSelected(_) => write!(f, "FileSelected(...)"),
            Message::FileLoaded(_) => write!(f, "FileLoaded(...)"),
            Message::DeviceLoaded(_) => write!(f, "DeviceLoaded(...)"),
            Message::DeviceData(_) => write!(f, "DeviceData(...)"),
            Message::ReadFailed(err) => write!(f, "ReadFailed({err})"),
            Message::RereadDevice(_, _) => write!(f, "RereadDevice"),
            Message::FlashFirmware => write!(f, "FlashFirmware"),
            Message::FlashComplete(_) => write!(f, "FlashComplete(...)"),
        }
    }
}

/// Detect device state
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum DetectState {
    #[default]
    Ice,
    Fire,
    Reread(McuVariant, FirmwareVersion),
    Done,
}

impl std::fmt::Display for DetectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectState::Ice => write!(f, "Ice"),
            DetectState::Fire => write!(f, "Fire"),
            DetectState::Reread(_, _) => write!(f, "Reread"),
            DetectState::Done => write!(f, "Done"),
        }
    }
}

impl DetectState {
    pub fn next(&self) -> Self {
        match self {
            DetectState::Ice => DetectState::Fire,
            DetectState::Fire => DetectState::Done,
            DetectState::Done => DetectState::Done,
            DetectState::Reread(_, _) => DetectState::Done,
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self, DetectState::Done)
    }

    /// We assume a specific STM32 MCU - doesn't matter which one as we're
    /// just readig common stuff, like flash base - and the chip ID will work
    /// for all.
    pub fn sample_mcu(&self) -> Option<McuVariant> {
        match self {
            DetectState::Ice => Some(McuVariant::F411RE),
            DetectState::Fire => Some(McuVariant::RP2350),
            DetectState::Reread(mcu, _) => Some(mcu.clone()),
            DetectState::Done => None,
        }
    }

    pub fn flash_base(&self) -> Option<u32> {
        self.sample_mcu().map(|mcu| mcu.family().get_flash_base())
    }

    pub fn chip_id(&self) -> Option<String> {
        self.sample_mcu().map(|mcu| mcu.chip_id().to_string())
    }
}

/// Analyse tab state
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum AnalyseState {
    #[default]
    Idle,
    Loading,
    Detecting(DetectState),
    Flashing,
}

impl AnalyseState {
    #[allow(dead_code)]
    pub fn is_busy(&self) -> bool {
        !self.is_idle()
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, AnalyseState::Idle)
    }
}

impl std::fmt::Display for AnalyseState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalyseState::Idle => write!(f, "Idle"),
            AnalyseState::Loading => write!(f, "Loading"),
            AnalyseState::Detecting(state) => write!(f, "Detecting ({})", state),
            AnalyseState::Flashing => write!(f, "Flashing"),
        }
    }
}

impl AnalyseState {
    pub fn content(&self) -> String {
        match self {
            AnalyseState::Idle => Analyse::ANALYSIS_TEXT_DEFAULT.to_string(),
            AnalyseState::Loading => "Loading firmware...".to_string(),
            AnalyseState::Detecting(state) => format!("Trying to detect One ROM {state} ..."),
            AnalyseState::Flashing => "Flashing firmware...".to_string(),
        }
    }
}

/// Analyse tab
#[derive(Debug, Clone)]
pub struct Analyse {
    selected_source_tab: SourceTab,
    analysis_content: String,
    fw_info: Option<SdrrInfo>,
    fw_file: Option<PathBuf>,
    file_contents: Option<Vec<u8>>,
    state: AnalyseState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SourceTab {
    Device,
    #[default]
    File,
}

impl std::fmt::Display for SourceTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceTab::Device => write!(f, "Device"),
            SourceTab::File => write!(f, "File"),
        }
    }
}

impl Default for Analyse {
    fn default() -> Self {
        Self {
            analysis_content: Self::ANALYSIS_TEXT_DEFAULT.to_string(),
            selected_source_tab: Default::default(),
            fw_info: Default::default(),
            fw_file: Default::default(),
            file_contents: Default::default(),
            state: Default::default(),
        }
    }
}

impl Analyse {
    // Button names
    const DEVICE_BUTTON_NAME: &'static str = "Device";
    const FILE_BUTTON_NAME: &'static str = "File";
    const SOURCE_DEVICE_BUTTON_NAME: &'static str = "Detect Device";
    const SOURCE_FILE_BUTTON_NAME: &'static str = "Select File";
    const ANALYSIS_TEXT_DEFAULT: &'static str = "No firmware analysed";
    const FLASH_BUTTON_NAME: &'static str = "Flash";

    pub const fn top_level_button_name() -> &'static str {
        "Analyse"
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, _runtime_info: &RuntimeInfo, message: Message) -> Task<AppMessage> {
        match message {
            Message::SourceTabSelected(tab) => {
                self.selected_source_tab = tab;
                Task::none()
            }
            Message::DetectDevice => {
                debug!("Starting device detection");
                // Clear out previous analysis content
                self.analysis_content = String::new();
                self.detect_device(None)
            }
            Message::SelectFile => {
                debug!("Selecting firmware file");
                self.fw_file_chooser()
            }
            Message::FileSelected(path) => {
                debug!("Firmware file selected: {:?}", path);
                self.load_file(path)
            }
            Message::FileLoaded(result) => {
                debug!("Firmware file loaded: {}", if result.is_ok() { "OK" } else { "Error" });
                self.file_device_loaded(result, true)
            }
            Message::DeviceLoaded(result) => {
                debug!("Device firmware loaded: {}", if result.is_ok() { "OK" } else { "Error" });
                self.file_device_loaded(result, false)
            }
            Message::DeviceData(data) => {
                debug!("Device data received: {} bytes", data.len());
                Task::future(Self::handle_device_data(data))
            }
            Message::ReadFailed(err) => {
                debug!("Device read failed: {}", err);
                // Move onto trying to detect next device type
                self.detect_device(Some(err))
            }
            Message::RereadDevice(mcu, fw_version) => {
                debug!("Re-reading device flash for MCU variant {} with fw v{}.{}.{}", mcu, fw_version.major(), fw_version.minor(), fw_version.patch());
                Task::done(self.reread_device(mcu, fw_version))
            }
            Message::FlashFirmware => {
                debug!("Flashing firmware to device");
                self.flash_firmware()
            }
            Message::FlashComplete(result) => {
                debug!("Firmware flash complete: {}", if result.is_ok() { "OK" } else { "Error" });
                self.firmware_flash_complete(result);
                Task::none()
            }
        }
    }

    fn flash_firmware(&mut self) -> Task<AppMessage> {
        if self.state.is_busy() {
            warn!("Cannot flash firmware - Analyse tab is busy ({})", self.state);
            self.analysis_content += "\nCannot flash firmware - Analyse tab is busy.\n";
            return Task::none();
        }

        self.state = AnalyseState::Flashing;

        if let Some(device_fw_data) = self.file_contents.as_ref() && let Some(filename) = self.fw_file.as_ref() {
            self.analysis_content = format!("Flashing {filename:?} to device...\n");
            Task::done(DeviceMessage::FlashFirmware(
                Client::Analyse,
                device_fw_data.clone(),
            ).into())
        } else {
            self.analysis_content = format!("Cannot flash - no file loaded\n");
            Task::none()
        }
    }

    fn firmware_flash_complete(&mut self, result: Result<(), String>) {
        if self.state != AnalyseState::Flashing {
            warn!("Received FlashComplete message while not flashing (state is {})", self.state);
            self.analysis_content += "\nReceived unexpected flash complete message.\n";
            return;
        }

        self.state = AnalyseState::Idle;

        match result {
            Ok(()) => {
                self.analysis_content += "\nFirmware flash completed successfully.\n";
            }
            Err(err) => {
                self.analysis_content += &format!("\nFirmware flash failed:\n- {err}\n");
            }
        }
    }

    fn reread_device(&mut self, mcu: McuVariant, fw_version: FirmwareVersion) -> AppMessage {
        // Indicate we're rereading
        debug!("Re-reading full flash for MCU variant {} with fw v{}.{}.{}", mcu, fw_version.major(), fw_version.minor(), fw_version.patch());
        self.analysis_content += &format!("\nRe-reading full flash from {mcu} based device with firmware v{}.{}.{}...\n", fw_version.major(), fw_version.minor(), fw_version.patch());
        self.state = AnalyseState::Detecting(DetectState::Reread(mcu.clone(), fw_version.clone()));

        // Build the message re-read the flash (and re-parse)
        let address = mcu.family().get_flash_base();
        let chip_id = mcu.chip_id().to_string();
        let words = mcu.flash_storage_bytes() / 4;
        DeviceMessage::ReadDevice {
            client: Client::Analyse,
            chip_id,
            address,
            words,
        }.into()
    }

    async fn handle_device_data(data: Vec<u8>) -> AppMessage {
        let data_copy = data.clone();
        let data_len = data.len();

        // Before proceeding, check if the entire data is 0xFF - this indicates
        // a blank flash
        if data.iter().all(|&b| b == 0xFF) {
            // There's no point in trying a longer (>64KB) read because we
            // don't know precisely what sort of device is being used, and
            // hence how much flash it has.  We'll assume it's entirely blank.
            debug!("Read flash data ({data_len} bytes) is all 0xFF - indicating blank flash");
            return Message::DeviceLoaded(Err("Blank device detected".to_string())).into();
        }

        // We always pass in 0x08000000 as the parser's base address even if
        // RP2350 - parser will figure out what
        // it's looking at
        let mut reader = MemoryReader::new(data, 0x08000000);
        let mut parser = Parser::new(&mut reader);
        let info = parser.parse_flash().await;
        let info = match info {
            Ok(info) => Ok((info, data_copy.to_vec())),
            Err(err) => Err(err),
        };

        // parse_flash() returns a Result<SdrrInfo, String>.
        // If the parsing worked, that's great, but we may still need to load and parse data
        // from the device again - as first time around we only read 64KB of flash, and in
        // pre-v0.5.0 firmware, often more than this is needed.
        if data_len > (64 * 1024) {
            // We read more than 64KB, so whatever happened just return the
            // result
            debug!("Firmware data length > 64KB ({} bytes), so not re-reading", data_len);
            Message::DeviceLoaded(info).into()
        } else {
            if let Err(err) = &info {
                // Parsing failed - just return the error
                debug!("Failed to parse firmware data: {}", err);
                return Message::DeviceLoaded(Err(err.clone())).into();
            }
            let (info, data) = info.unwrap();

            if info.version >= FW_VERSION_0_5_0 || info.parse_errors.is_empty() {
                // Firmware is v0.5.0 or later, so 64KB read is sufficient, or
                // we parsed everything OK anyway
                trace!("Firmware is v0.5.0 or later, or parsed successfully");
                return Message::DeviceLoaded(Ok((info, data))).into();
            }

            if info.mcu_variant.is_none() {
                // The MCU info wasn't decoded.  This is worrying, and means
                // we can't confidently predict the size, so just return as is.
                info!("MCU variant {} {} not detected during firmware decode, cannot re-read full flash", info.stm_line, info.stm_storage);
                return Message::DeviceLoaded(Ok((info, data))).into();
            }
            let mcu = info.mcu_variant.unwrap();

            // Ready to re-read full flash
            Message::RereadDevice(mcu, info.version).into()
        }

    }

    fn detect_device(&mut self, err: Option<String>) -> Task<AppMessage> {
        self.file_contents = None;

        if let Some(err) = err {
            self.fw_info = None;
            self.analysis_content += &format!("\nError reading from device:\n- {err}\n");
        }

        // Move onto next detection state
        let new_state = match &self.state {
            AnalyseState::Detecting(state) => AnalyseState::Detecting(state.next()),
            _ => AnalyseState::Detecting(DetectState::default()),
        };
        let detect_state = match new_state.clone() {
            AnalyseState::Detecting(state) => state,
            _ => unreachable!(),
        };

        if detect_state.is_done() {
            self.fw_info = None;
            self.analysis_content += "---\nDevice detection failed - neither One ROM Ice nor One ROM Fire hardware detected.\nHave you connected the probe to the One ROM correctly, and does the One ROM have power?";
            self.state = AnalyseState::Idle;
            return Task::none();
        }

        // Actually do a detection, based on current state
        let start_analysis_task = self.start_analysis(new_state);
        let read_device_task = Task::done(AppMessage::Device(DeviceMessage::ReadDevice {
            client: Client::Analyse,
            chip_id: detect_state.chip_id().expect("Chip ID should be available"),
            address: detect_state
                .flash_base()
                .expect("Flash base should be available"),
            words: 65536 / 4,
        }));

        Task::chain(start_analysis_task, read_device_task)
    }

    fn file_device_loaded(&mut self, result: Result<(SdrrInfo, Vec<u8>), String>, is_file: bool) -> Task<AppMessage> {
        match result {
            Ok((info, data)) => {
                let json = serde_json::to_string_pretty(&info).map_err(|e| e.to_string());
                self.analysis_content = match json {
                    Ok(j) => j,
                    Err(e) => format!("Error serializing info to JSON: {}", e),
                };
                self.fw_info = Some(info);
                self.file_contents = if is_file {
                    Some(data)
                } else {
                    None
                };
            }
            Err(err) => {
                self.fw_info = None;
                self.analysis_content = if is_file {
                    format!(
                        "Error loading/parsing file:\n- {}\n---\nAre you sure this is a valid One ROM firmware .bin file?",
                        err,
                    )
                } else {
                    format!(
                        "Error loading/parsing device firmware:\n- {}\n---\nAre you sure this device is a previously programmed One ROM?",
                        err,
                    )
                }
            }
        }
        self.state = AnalyseState::Idle;

        // Send decoded hardware informaton to the rest of the app
        match self.share_hw_info() {
            Some(msg) => Task::done(msg),
            None => Task::none(),
        }
    }

    fn share_hw_info(&mut self) -> Option<AppMessage> {
        if let Some(info) = self.fw_info.as_ref() {
            let hw_info = HardwareInfo {
                board: info.board,
                model: info.model,
                mcu_variant: info.mcu_variant,
            };
            Some(AppMessage::Studio(StudioMessage::HardwareInfo(Some(
                hw_info,
            ))))
        } else {
            None
        }
    }

    fn clear_hw_info(&self) -> Task<AppMessage> {
        Task::done(StudioMessage::HardwareInfo(None).into())
    }

    fn start_analysis(&mut self, state: AnalyseState) -> Task<AppMessage> {
        self.state = state;
        self.analysis_content += &self.state.content().to_string();
        self.fw_info = None;
        self.file_contents = None;
        self.clear_hw_info()
    }

    fn load_file(&mut self, path: Option<PathBuf>) -> Task<AppMessage> {
        if let Some(path) = path {
            let start_analysis_task = self.start_analysis(AnalyseState::Loading);
            self.fw_file = Some(path.clone());
            let load_file_task =
                Task::perform(async move { Self::async_load_file(path).await }, |result| {
                    AppMessage::Analyse(Message::FileLoaded(result))
                });
            Task::batch([start_analysis_task, load_file_task])
        } else {
            Task::none()
        }
    }

    async fn async_load_file(path: PathBuf) -> Result<(SdrrInfo, Vec<u8>), String> {
        if path.exists() && path.is_file() {
            // Read in the file
            let data = std::fs::read(path).map_err(|e| e.to_string())?;

            // Parse it
            let mut reader = MemoryReader::new(data.clone(), 0x08000000);
            let mut parser = Parser::new(&mut reader);
            let parser_result = parser.parse_flash().await;
            parser_result.map(|info| (info, data))
        } else {
            Err("File does not exist or is a directory".to_string())
        }
    }

    fn fw_file_chooser(&self) -> Task<AppMessage> {
        Task::perform(
            async {
                FileDialog::new()
                    .add_filter("firmware", &["bin"])
                    .pick_file()
            },
            |path| Message::FileSelected(path).into(),
        )
    }

    pub fn view(&self, runtime_info: &RuntimeInfo, device: &Device) -> Element<'_, AppMessage> {
        let hw_info = runtime_info.hw_info();

        let buttons = row![
            self.fw_source_buttons(),
            Space::with_width(Length::Fill),
            self.fw_source_control(device),
        ].align_y(iced::alignment::Vertical::Center);

        column![
            column![
                self.select_fw_source(),
                buttons,
                Style::horiz_line(),
                self.fw_content_heading(hw_info),
            ]
            .spacing(20),
            Space::with_height(Length::Fixed(20.0)),
            Style::container(self.fw_content()),
        ]
        .into()
    }

    fn select_fw_source(&self) -> Element<'_, AppMessage> {
        row![Style::text_h3("Select Firmware Source")].into()
    }

    fn fw_source_buttons(&self) -> Element<'_, AppMessage> {
        // Determine button states based on selected tab
        let is_file_selected = matches!(self.selected_source_tab, SourceTab::File);

        let file_message = if is_file_selected {
            None
        } else {
            if self.state.is_idle() {
                Some(Message::SourceTabSelected(
                    SourceTab::File,
                ).into())
            } else {
                None
            }
        };

        let device_message = if is_file_selected {
            if self.state.is_idle() {
                Some(Message::SourceTabSelected(
                    SourceTab::Device,
                ).into())
            } else {
                None
            }
        } else {
            None
        };

        let file_button =
            Style::text_button_small(Self::FILE_BUTTON_NAME, file_message, is_file_selected);

        let device_button =
            Style::text_button_small(Self::DEVICE_BUTTON_NAME, device_message, !is_file_selected);

        row![file_button, device_button]
            .spacing(20)
            .into()
    }

    fn fw_source_control(&self, device: &Device) -> Element<'_, AppMessage> {
        let source_button = match self.selected_source_tab {
            SourceTab::Device => self.fw_source_device_control(device),
            SourceTab::File => self.fw_source_file_control(),
        };

        let row = row![];

        // Show flash file if on file source tab
        if self.selected_source_tab == SourceTab::File && self.file_contents.is_some(){
            row.push(self.flash_file_button(device))
        } else {
            row
        }
            .push(source_button)
            .spacing(20).into()
    }

    fn flash_file_button(&self, device: &Device) -> Button<'_, AppMessage> {
        let highlighted = if self.state.is_idle() && !device.selected().is_none() && device.is_idle() && self.fw_info.is_some() {
            true
        } else {
            false
        };

        let message = if self.state.is_idle() && !device.selected().is_none() && self.fw_info.is_some() && !device.is_busy() {
            Some(Message::FlashFirmware.into())
        } else {
            None
        };

        let content = if self.state.is_idle() {
            Self::FLASH_BUTTON_NAME
        } else {
            "Flashing..."
        };

        Style::text_button_small(content, message, highlighted)
    }

    fn fw_source_device_control(&self, device: &Device) -> Button<'_, AppMessage> {
        let highlighted = if self.state.is_idle() && !device.selected().is_none() {
            true
        } else {
            false
        };

        let message = if self.state.is_idle() && !device.selected().is_none() {
            Some(AppMessage::Analyse(Message::DetectDevice))
        } else {
            None
        };

        let content = if self.state.is_idle() {
            Self::SOURCE_DEVICE_BUTTON_NAME
        } else {
            "Detecting..."
        };

        Style::text_button_small(content, message, highlighted)
    }

    fn fw_source_file_control(&self) -> Button<'_, AppMessage> {
        // Only enable this button if file not being loaded
        let file_control_message = if self.state.is_idle() {
            Some(Message::SelectFile.into())
        } else {
            None
        };

        let content = if self.state != AnalyseState::Loading {
            Self::SOURCE_FILE_BUTTON_NAME
        } else {
            "Loading..."
        };

        let highlight = self.state.is_idle();

        // Create the button
        Style::text_button_small(content, file_control_message, highlight)
    }

    fn fw_content_heading(&self, hw_info: Option<&HardwareInfo>) -> Element<'_, AppMessage> {
        // Include hardware info if available
        let heading = Style::text_h3("Analysis");
        if let Some(hw_info) = hw_info {
            let version = self.fw_info.as_ref().and_then(|info| Some(info.version));
            let metadata = self.fw_info.as_ref().map_or(Some(false), |info| Some(info.metadata_present));
            let info_row = Style::hw_info_row(
                version,
                metadata,
                hw_info.model,
                hw_info.board,
                hw_info.mcu_variant,
                false,
            );

            row![heading, Space::with_width(Length::Fill), info_row,]
                .align_y(iced::alignment::Vertical::Center)
        } else {
            row![heading]
        }
        .into()
    }

    fn fw_content(&self) -> Element<'_, AppMessage> {
        Style::box_scrollable_text(&self.analysis_content, 350.0, true).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}
