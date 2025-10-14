// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use onerom_config::fw::FirmwareVersion;
use onerom_config::hw::Board as HwBoard;
use onerom_config::mcu::Variant as McuVariant;

use crate::Error;

pub const FIRMWARE_SITE_BASE: &str = "images.onerom.org";
pub const FIRMWARE_RELEASE_MANIFEST: &str = "releases.json";

/// Retrieves a license from a URL
pub fn fetch_license(url: &str) -> Result<String, Error> {
    debug!("Fetching license from {}", url);
    let response = reqwest::blocking::get(url).map_err(Error::network)?;
    let body = response.text().map_err(Error::network)?;
    Ok(body)
}

/// Retrieves a ROM file from a URL, extracting it from a zip file if needed
pub fn fetch_rom_file(url: &str, extract: Option<String>) -> Result<Vec<u8>, Error> {
    // Get the file itself
    debug!("Fetching ROM file from {}", url);
    let response = reqwest::blocking::get(url).map_err(Error::network)?;
    let bytes = response.bytes().map_err(Error::network)?;

    // Now extract if needed
    if let Some(extract) = extract {
        debug!("Extracting file `{}` from zip", extract);
        let reader = std::io::Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(reader).map_err(Error::zip)?;
        let mut file = zip.by_name(&extract).map_err(Error::zip)?;
        let mut data = Vec::new();
        std::io::copy(&mut file, &mut data).map_err(Error::read)?;
        Ok(data)
    } else {
        Ok(bytes.to_vec())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Releases {
    pub latest: String,
    releases: Vec<Release>,
}

impl Releases {
    pub fn manifest_url() -> String {
        format!("https://{}/{}", FIRMWARE_SITE_BASE, FIRMWARE_RELEASE_MANIFEST)
    }

    pub fn from_network() -> Result<Self, Error> {
        let url = Self::manifest_url();
        debug!("Fetching releases manifest from {}", url);
        let response = reqwest::blocking::get(&url).map_err(Error::network)?;
        let body = response.text().map_err(Error::network)?;
        Self::from_json(&body)
    }

    pub fn from_json(data: &str) -> Result<Releases, Error> {
        serde_json::from_str(data).map_err(Error::json)
    }

    pub fn version_str(version: &FirmwareVersion) -> String {
        format!("{}.{}.{}", version.major(), version.minor(), version.patch())
    }

    pub fn release(&self, version: &FirmwareVersion) -> Option<&Release> {
        let version = Self::version_str(version);
        self.releases.iter().find(|r| r.version == version)
    }

    pub fn releases(&self) -> &Vec<Release> {
        &self.releases
    }

    pub fn releases_str(&self) -> String {
        self.releases.iter().map(|r| r.version.as_str()).collect::<Vec<_>>().join(", ")
    }

    pub fn latest(&self) -> &str {
        &self.latest
    }

    pub fn download_firmware(&self, version: &FirmwareVersion, board: &HwBoard, mcu: &McuVariant) -> Result<Vec<u8>, Error> {
        let board = board.name();
        let mcu = mcu.to_string();

        // Get the release
        let release = self.release(version).ok_or_else(|| {
            debug!("Failed to find release for {version:?}");
            Error::release_not_found()
        })?;

        // Get the firmware path
        let path = release.path(board, &mcu)?;
        let url = format!("https://{}/{}/firmware.bin", FIRMWARE_SITE_BASE, path);

        // Download the firmware
        debug!("Downloading firmware from {}", url);
        let response = reqwest::blocking::get(&url).map_err(Error::network)?;
        let bytes = response.bytes().map_err(Error::network)?;
        Ok(bytes.to_vec())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Release {
    pub version: String,
    pub path: Option<String>,
    pub notes: Option<String>,
    pub boards: Vec<Board>,
}

impl Release {
    fn path(&self, board: &str, mcu: &str) -> Result<String, Error> {
        let board = self.board(&board.to_ascii_lowercase()).ok_or_else(|| {
            debug!("Failed to find board for {board:?}");
            Error::release_not_found()
        })?;
        let path = self.path.clone().unwrap_or_else(|| self.version.clone());

        Ok(format!("{path}/{}", board.path(mcu)?))
    }

    fn board(&self, board: &str) -> Option<&Board> {
        self.boards.iter().find(|b| b.name == board)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Board {
    pub name: String,
    pub path: Option<String>,
    pub mcus: Vec<Mcu>,
}

impl Board {
    fn path(&self, mcu: &str) -> Result<String, Error> {
        let mcu = self.mcu(&mcu.to_ascii_lowercase()).ok_or_else(|| {
            debug!("Failed to find MCU for {mcu:?}");
            Error::release_not_found()
        })?;
        let path = self.path.clone().unwrap_or_else(|| self.name.clone());

        Ok(format!("{path}/{}", mcu.path()))
    }

    fn mcu(&self, mcu: &str) -> Option<&Mcu> {
        self.mcus.iter().find(|m| m.name == mcu)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Mcu {
    name: String,
    path: Option<String>,
}

impl Mcu {
    fn path(&self) -> String {
        self.path.clone().unwrap_or_else(|| self.name.clone())
    }
}