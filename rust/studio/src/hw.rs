// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! Contains One ROM hardware related types and functions

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use onerom_config::fw::{FirmwareProperties, ServeAlg};
use onerom_config::hw::{Board, Model};
use onerom_config::mcu::Variant as McuVariant;
use onerom_fw::net::Release;

/// Information about hardware
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HardwareInfo {
    pub board: Option<Board>,
    pub model: Option<Model>,
    pub mcu_variant: Option<McuVariant>,
}

impl HardwareInfo {
    pub fn is_complete(&self) -> bool {
        self.board.is_some() && self.model.is_some() && self.mcu_variant.is_some()
    }

    pub fn firmware_properties(&self, release: &Release) -> Option<FirmwareProperties> {
        let version = match release.firmware_version() {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to get firmware version: {e}");
                return None;
            }
        };

        // Get other values
        let board = match self.board {
            Some(b) => b,
            None => {
                warn!("Cannot get board for firmware properties");
                return None;
            }
        };
        let variant = match self.mcu_variant {
            Some(v) => v,
            None => {
                warn!("Cannot get MCU variant for firmware properties");
                return None;
            }
        };
        let serve_alg = ServeAlg::default();
        let boot_logging = true;

        match FirmwareProperties::new(
            version,
            board,
            variant,
            serve_alg,
            boot_logging,
        ) {
            Ok(props) => Some(props),
            Err(e) => {
                warn!("Cannot create firmware properties: {e:?}");
                None
            }
        }
    }

    pub fn board_name(&self) -> String {
        self.board.as_ref().map(|b| b.name().to_string()).unwrap_or_else(|| "unknown".into())
    }

    pub fn mcu_name(&self) -> String {
        self.mcu_variant.as_ref().map(|m| m.to_string()).unwrap_or_else(|| "unknown".into()).to_lowercase()
    }
}

impl std::fmt::Display for HardwareInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HardwareInfo: board={:?}, model={:?}, mcu_variant={:?}",
            self.board.as_ref().map(|b| b.name()),
            self.model.as_ref().map(|m| m.name()),
            self.mcu_variant
        )
    }
}
