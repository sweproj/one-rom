// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

use iced::widget::Row;
use iced::{Element, Subscription, Task, time};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::time::Duration;

use onerom_config::fw::FirmwareVersion;
use onerom_config::hw::Board;
use onerom_config::mcu::Variant as McuVariant;
use onerom_fw::net::{fetch_rom_file_async, Release, Releases};
use onerom_gen::{Builder, FileData, FIRMWARE_SIZE, MAX_METADATA_LEN};

use crate::analyse::Analyse;
use crate::app::AppMessage;
use crate::config::{Configs, get_config_from_url};
use crate::create::{Create, Message as CreateMessage};
use crate::hw::HardwareInfo;
use crate::log::Log;
use crate::style::Style;
use crate::task_from_msg;

const MANIFEST_RETRY_SHORT: Duration = Duration::from_secs(10);
const MANIFEST_RETRY_LONG: Duration = Duration::from_secs(60);

/// Messages for main window
#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(StudioTab),
    HardwareInfo(Option<HardwareInfo>),
    FetchReleases,
    Releases(Releases),
    DownloadRelease(Release, Board, McuVariant),
    ReleaseDownloaded(Vec<u8>),
    ClearDownloadedRelease,
    FetchConfigs,
    Configs(Configs),
    DownloadConfig(String),
    ConfigDownloaded(Vec<u8>),
    ClearDownloadedConfig,
    BuildImages(HardwareInfo),
    BuildImagesResult(Result<(Images, String), String>),
    HelpPressed,
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::TabSelected(tab) => write!(f, "TabSelected({tab})"),
            Message::HardwareInfo(info) => write!(f, "HardwareInfo({info:?})"),
            Message::FetchReleases => write!(f, "FetchReleases"),
            Message::Releases(releases) => write!(f, "Releases({})  ", releases.releases_str()),
            Message::DownloadRelease(release, board, mcu) => {
                write!(f, "DownloadRelease({}, {board}, {mcu})", release.version)
            }
            Message::ReleaseDownloaded(data) => {
                write!(f, "ReleaseDownloaded({} bytes)", data.len())
            }
            Message::ClearDownloadedRelease => write!(f, "ClearDownloadedRelease"),
            Message::FetchConfigs => write!(f, "FetchConfigs"),
            Message::Configs(configs) => write!(f, "Configs({})", configs.names_str()),
            Message::DownloadConfig(name) => write!(f, "DownloadConfig({name})"),
            Message::ConfigDownloaded(data) => {
                write!(f, "ConfigDownloaded({} bytes)", data.len())
            }
            Message::ClearDownloadedConfig => write!(f, "ClearDownloadedConfig"),
            Message::BuildImages(hw) => write!(f, "BuildImages({hw})"),
            Message::BuildImagesResult(_) => write!(f, "BuildImagesResult"),
            Message::HelpPressed => write!(f, "HelpPressed"),
        }
    }
}

/// Tabs for main window
#[derive(Debug, Default, Clone, PartialEq)]
pub enum StudioTab {
    #[default]
    Analyse,
    Create,
    Log,
}

impl std::fmt::Display for StudioTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StudioTab::Create => write!(f, "Create"),
            StudioTab::Analyse => write!(f, "Analyse"),
            StudioTab::Log => write!(f, "Log"),
        }
    }
}

impl StudioTab {
    /// Get the tab name
    pub fn name(&self) -> &str {
        match self {
            StudioTab::Create => Create::top_level_button_name(),
            StudioTab::Analyse => Analyse::top_level_button_name(),
            StudioTab::Log => Log::top_level_button_name(),
        }
    }

    /// Create the tab buttons
    ///
    /// Returns a row of buttons
    pub fn buttons(active: &StudioTab, serious_errors: bool) -> Element<'_, AppMessage> {
        let mut buttons = Vec::new();
        for tab in vec![StudioTab::Analyse, StudioTab::Create, StudioTab::Log] {
            let active = *active == tab;
            let on_press = if active {
                None
            } else {
                Some(AppMessage::Studio(Message::TabSelected(tab.clone())))
            };
            let button = if serious_errors && tab == StudioTab::Log {
                Style::error_button(tab.name(), on_press, active)
            } else {
                Style::text_button(tab.name(), on_press, active)
            };
            buttons.push(button.into());
        };
        
        // Add the buttons to a row, with spacing between them
        Row::with_children(buttons).spacing(20).into()
    }   


}

/// Images built by Studio
#[derive(Debug, Clone)]
pub struct Images {
    firmware: Vec<u8>,

    metadata: Vec<u8>,

    roms: Vec<u8>,
}

impl std::fmt::Display for Images {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Images({}/{}/{}",
            self.firmware.len(),
            self.metadata.len(),
            self.roms.len()
        )
    }
}

#[allow(dead_code)]
impl Images {
    /// Returns just the required portion of the firmware image
    pub fn firmware_skinny(&self) -> &[u8] {
        &self.firmware
    }

    /// Returns the length of the firmware portion
    pub fn firmware_len(&self) -> usize {
        self.firmware.len()
    }

    /// Returns the maximum firmware length (48KB)
    pub const fn max_firmware_len() -> usize {
        FIRMWARE_SIZE
    }

    /// Returns offset from start of flash the firmware is located
    pub const fn firmware_offset() -> usize {
        0
    }

    /// Returns the full firmware portion padded to 48KB
    pub fn firmware_full(&self) -> Vec<u8> {
        let mut fw = vec![0xFF_u8; Self::max_firmware_len()];
        let skinny = self.firmware_skinny();
        fw[..skinny.len()].copy_from_slice(skinny);
        fw
    }

    /// Returns the metadata portion
    pub fn metadata_skinny(&self) -> &[u8] {
        &self.metadata
    }

    /// Returns the length of the metadata portion
    pub fn metadata_len(&self) -> usize {
        self.metadata.len()
    }

    /// Returns maximum metadata length (16KB)
    pub const fn max_metadata_len() -> usize {
        MAX_METADATA_LEN
    }

    /// Returns offset from start of flash the metadata is located
    pub const fn metadata_offset() -> usize {
        FIRMWARE_SIZE
    }

    /// Returns the full metadata portion padded to 16KB
    pub fn metadata_full(&self) -> Vec<u8> {
        let mut md = vec![0xFF_u8; Self::max_metadata_len()];
        let skinny = self.metadata_skinny();
        md[..skinny.len()].copy_from_slice(skinny);
        md
    }

    /// Returns the ROMs portion
    pub fn roms(&self) -> &[u8] {
        &self.roms
    }

    /// Returns the length of the ROMs portion
    pub fn roms_len(&self) -> usize {
        self.roms.len()
    }

    /// Returns offset from start of flash the ROMs are located
    pub const fn roms_offset() -> usize {
        FIRMWARE_SIZE + MAX_METADATA_LEN
    }

    /// Returns full image as would be flashed to device
    pub fn full_image(&self) -> Vec<u8> {
        let mut image = Vec::new();
        image.extend_from_slice(&self.firmware_full());
        image.extend_from_slice(&self.metadata_full());
        image.extend_from_slice(&self.roms);
        image
    }

    /// Returns size of full image as would be flashed to device
    pub fn full_image_len(&self) -> usize {
        Self::max_firmware_len() + Self::max_metadata_len() + self.roms_len()
    }
}

/// Contains information retrieved/computed at runtime
#[derive(Debug, Clone, Default)]
pub struct RuntimeInfo {
    // One ROM releases retrieved from network
    releases: Option<Releases>,

    // Detected or selected hardware info
    hw_info: Option<HardwareInfo>,

    // Downloaded firmware image
    firmware: Option<Vec<u8>>,

    // Selected firmware
    selected_firmware: Option<Release>,

    // Available configs
    configs: Option<Configs>,

    // Downloaded config
    config: Option<Vec<u8>>,

    // Selected config
    selected_config: Option<String>,

    // Built images
    images: Option<Images>,
}

impl RuntimeInfo {
    pub fn releases(&self) -> Option<&Releases> {
        self.releases.as_ref()
    }

    fn set_releases(&mut self, releases: Releases) {
        self.releases = Some(releases);
    }

    fn set_configs(&mut self, configs: Configs) {
        self.configs = Some(configs);
    }

    pub fn hw_info(&self) -> Option<&HardwareInfo> {
        self.hw_info.as_ref()
    }

    fn set_hw_info(&mut self, hw_info: Option<HardwareInfo>) {
        self.hw_info = hw_info;
    }

    #[allow(dead_code)]
    pub fn firmware(&self) -> Option<&Vec<u8>> {
        self.firmware.as_ref()
    }

    pub fn firmware_len(&self) -> Option<usize> {
        self.firmware.as_ref().map(|f| f.len())
    }

    pub fn config_len(&self) -> Option<usize> {
        self.config.as_ref().map(|c| c.len())
    }

    fn set_firmware(&mut self, firmware: Vec<u8>) {
        self.firmware = Some(firmware);
    }

    fn clear_firmware(&mut self) {
        self.firmware = None;
    }

    pub fn selected_firmware(&self) -> Option<&Release> {
        self.selected_firmware.as_ref()
    }

    fn set_selected_firmware(&mut self, release: Release) {
        self.selected_firmware = Some(release);
    }

    fn clear_selected_firmware(&mut self) {
        self.selected_firmware = None;
        self.firmware = None;
    }

    pub fn configs(&self) -> Option<&Configs> {
        self.configs.as_ref()
    }

    fn set_config(&mut self, config: Vec<u8>) {
        self.config = Some(config);
    }

    pub fn clear_config(&mut self) {
        self.config = None;
    }

    pub fn selected_config(&self) -> Option<&String> {
        self.selected_config.as_ref()
    }

    fn set_selected_config(&mut self, name: String) {
        self.selected_config = Some(name);
    }

    fn clear_selected_config(&mut self) {
        self.selected_config = None;
        self.config = None;
    }

    pub fn config(&self) -> Option<&Vec<u8>> {
        self.config.as_ref()
    }

    /// Returns reference to images
    pub fn images(&self) -> Option<&Images> {
        self.images.as_ref()
    }

    pub fn built_firmware_len(&self) -> Option<usize> {
        self.images().map(|imgs| imgs.firmware_len())
    }

    pub fn built_metadata_len(&self) -> Option<usize> {
        self.images().map(|imgs| imgs.metadata_len())
    }

    pub fn built_roms_len(&self) -> Option<usize> {
        self.images().map(|imgs| imgs.roms_len())
    }

    pub fn built_full_image_len(&self) -> Option<usize> {
        self.images().map(|imgs| imgs.full_image_len())
    }
}

/// Main application state
#[derive(Debug, Default, Clone)]
pub struct Studio {
    active_tab: StudioTab,
    runtime_info: RuntimeInfo,
}

impl Studio {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_tab(&self) -> &StudioTab {
        &self.active_tab
    }

    pub fn runtime_info(&self) -> &RuntimeInfo {
        &self.runtime_info
    }

    pub fn update(&mut self, message: Message) -> Task<AppMessage> {
        match message {
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                Task::none()
            }
            Message::HardwareInfo(info) => {
                self.runtime_info.set_hw_info(info.clone());

                // Share with Create
                task_from_msg!(CreateMessage::DetectedHardwareInfo)
            }
            Message::FetchReleases => Task::future(Self::fetch_releases_async()),
            Message::Releases(releases) => {
                self.runtime_info.set_releases(releases.clone());
                Task::done(CreateMessage::ReleasesUpdated.into())
            }
            Message::DownloadRelease(release, board, mcu) => {
                self.download_release(release, board, mcu)
            }
            Message::ReleaseDownloaded(data) => {
                self.runtime_info.set_firmware(data.clone());
                Task::none()
            }
            Message::ClearDownloadedRelease => {
                self.runtime_info.clear_firmware();
                Task::none()
            }
            Message::FetchConfigs => {
                Task::future(Self::fetch_configs_async())
            }
            Message::Configs(configs) => {
                self.runtime_info.set_configs(configs.clone());
                Task::done(CreateMessage::ConfigsUpdated.into())
            }
            Message::DownloadConfig(name) => {
                self.download_config(name)
            }
            Message::ConfigDownloaded(data) => {
                self.runtime_info.set_config(data.clone());
                Task::none()
            }
            Message::ClearDownloadedConfig => {
                self.runtime_info.clear_config();
                Task::none()
            }
            Message::BuildImages(hw_info) => {
                Task::future(Self::build_images_async(hw_info, self.runtime_info.clone()))
            }
            Message::BuildImagesResult(result) => {
                let msg = match result {
                    Ok((images, desc)) => {
                        self.runtime_info.images = Some(images);
                        CreateMessage::BuildImagesResult(Ok(desc))
                    }
                    Err(e) => {
                        warn!("Failed to build images: {e}");
                        CreateMessage::BuildImagesResult(Err(e))
                    }
                };
                Task::done(msg.into())
            }
            Message::HelpPressed => self.help_pressed(),
        }
    }

    fn help_pressed(&self) -> Task<AppMessage> {
        Task::none()
    }

    async fn build_images_async(hw_info: HardwareInfo, runtime_info: RuntimeInfo) -> AppMessage {
        // Check we have firmware and config
        let firmware = if let Some(fw) = runtime_info.firmware() {
            fw.clone()
        } else {
            warn!("No firmware downloaded, cannot build images");
            return CreateMessage::BuildImagesResult(Err("No firmware downloaded".to_string())).into();
        };

        let config = if let Some(cfg) = runtime_info.config() {
            cfg.clone()
        } else {
            warn!("No config downloaded, cannot build images");
            return CreateMessage::BuildImagesResult(Err("No config downloaded".to_string())).into();
        };

        // Turn config into string
        let config_str = match String::from_utf8(config.clone()) {
            Ok(s) => s,
            Err(e) => {
                warn!("Config is not valid UTF-8: {}", e);
                return CreateMessage::BuildImagesResult(Err("Config is not valid UTF-8".to_string())).into();
            }
        };

        // Create image builder from config
        let mut builder = match Builder::from_json(&config_str) {
            Ok(b) => b,
            Err(e) => {
                warn!("Failed to create image builder from config: {e:?}");
                return CreateMessage::BuildImagesResult(Err(format!("Failed to create image builder from config:\n  - {e:?}")).into()).into();
            }
        };

        // Get ROM files we need to download
        let file_specs = builder.file_specs();
        for spec in file_specs {
            let id = spec.id;
            let url = &spec.source;
            let extract = spec.extract;
            debug!("Downloading ROM file from {url} (extract={extract:?})");
            match fetch_rom_file_async(url, extract).await {
                Ok(data) => {
                    info!("Downloaded ROM file {url} ({} bytes)", data.len());

                    // Give it to the builder
                    let data = FileData {
                        id,
                        data,
                    };
                    
                    match builder.add_file(data) {
                        Ok(()) => {
                            trace!("Added ROM file {url} to builder");
                        }
                        Err(e) => {
                            warn!("Failed to add ROM file {url} to builder: {e:?}");
                            return CreateMessage::BuildImagesResult(Err(format!("Failed to add ROM file {url} to builder:\n  - {e:?}"))).into();
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to download ROM file {}: {}", url, e);
                    return CreateMessage::BuildImagesResult(Err(format!("Failed to download ROM file {url}:\n  - {e}"))).into();
                }
            };
        }

        // Get firmware version
        let fw = match runtime_info.selected_firmware() {
            Some(fw) => fw,
            None => {
                warn!("No selected firmware, cannot build images");
                return CreateMessage::BuildImagesResult(Err("No selected firmware".to_string())).into();
            }
            
        };

        // Build the firmware properties
        let props = match hw_info.firmware_properties(&fw) {
            Some(p) => p,
            None => {
                warn!("Cannot get firmware properties, cannot build images");
                return CreateMessage::BuildImagesResult(Err("Cannot get firmware properties".to_string())).into();
            }
        };

        // Build the images
        let (metadata, roms) = match builder.build(props) {
            Ok((md, roms)) => (md, roms),
            Err(e) => {
                warn!("Failed to build images: {e:?}");
                return CreateMessage::BuildImagesResult(Err(format!("Failed to build images:\n  - {e:?}"))).into();
            }
        };

        // Store images
        let images = Images {
            firmware,
            metadata,
            roms,
        };
        let total_len = images.full_image_len();
        let fw_len = images.firmware_len();
        let md_len = images.metadata_len();
        let roms_len = images.roms_len();

        // Get description
        let desc = builder.description();

        info!(
            "Built images: total={total_len} bytes, firmware={fw_len} bytes, metadata={md_len} bytes, roms={roms_len} bytes"
        );

        Message::BuildImagesResult(Ok((images,desc))).into()
    }

    async fn fetch_releases_async() -> AppMessage {
        match Releases::from_network_async().await {
            Ok(releases) => AppMessage::Studio(Message::Releases(releases)),
            Err(e) => {
                warn!("Failed to fetch releases from network\n  - {e}");
                AppMessage::Nop
            }
        }
    }

    async fn fetch_configs_async() -> AppMessage {
        match Configs::from_network_async().await {
            Ok(configs) => AppMessage::Studio(Message::Configs(configs)),
            Err(e) => {
                warn!("Failed to fetch configs from network\n  - {e}");
                AppMessage::Nop
            }
        }
    }

    fn download_release(
        &mut self,
        release: Release,
        board: Board,
        mcu: McuVariant,
    ) -> Task<AppMessage> {
        self.runtime_info.clear_selected_firmware();

        // Check we have Releases
        let releases = if let Some(releases) = self.runtime_info.releases() {
            releases.clone()
        } else {
            error!("No releases available in Studio, cannot download");
            return Task::none();
        };

        // Get the firmware version
        let Ok(fw_ver) = release.firmware_version() else {
            warn!("No firmware version {release} found, cannot download");
            return Task::none();
        };

        // Set the selected firmware
        self.runtime_info.set_selected_firmware(release.clone());

        // Download the firmware
        Task::future(Self::download_release_async(releases, fw_ver, board, mcu))
    }

    fn download_config(&mut self, name: String) -> Task<AppMessage> {
        self.runtime_info.clear_selected_config();

        // Check we have Configs and get the config URL
        let config_url  = if let Some(configs) = self.runtime_info.configs() {
            match configs.config_url(&name) {
                Some(url) => url,
                None => {
                    warn!("No config named {name} found, cannot download");
                    return Task::none();
                }
            }
        } else {
            error!("No configs available in Studio, cannot download");
            return Task::none();
        };

        // Set the selected config
        self.runtime_info.set_selected_config(name.clone());

        // Download the config
        Task::future(Self::download_config_async(config_url))
    }

    async fn download_release_async(
        releases: Releases,
        fw_ver: FirmwareVersion,
        board: Board,
        mcu: McuVariant,
    ) -> AppMessage {
        // Download the firmware
        match releases
            .download_firmware_async(&fw_ver, &board, &mcu)
            .await
        {
            Ok(data) => Message::ReleaseDownloaded(data).into(),
            Err(e) => {
                warn!("Failed to download firmware: {}", e);
                AppMessage::Nop
            }
        }
    }

    async fn download_config_async(path: String) -> AppMessage {
        // Download the config
        match get_config_from_url(&path).await {
            Ok(data) => Message::ConfigDownloaded(data).into(),
            Err(e) => {
                warn!("Failed to download config: {}", e);
                AppMessage::Nop
            }
        }
    }

    pub fn top_level_buttons(&self, serious_errors: bool) -> iced::Element<'_, AppMessage> {
        StudioTab::buttons(&self.active_tab(), serious_errors)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let check_releases_duration = if self.runtime_info.releases().is_some() {
            MANIFEST_RETRY_LONG
        } else {
            MANIFEST_RETRY_SHORT
        };
        let check_configs_duration = if self.runtime_info.configs().is_some() {
            MANIFEST_RETRY_LONG
        } else {
            MANIFEST_RETRY_SHORT
        };

        Subscription::batch(vec![
            time::every(check_releases_duration).map(|_| Message::FetchReleases),
            time::every(check_configs_duration).map(|_| Message::FetchConfigs),
        ])
    }
}
