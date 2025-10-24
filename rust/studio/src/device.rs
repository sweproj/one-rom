// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

use dfu_rs::{DeviceInfo as DfuDeviceInfo, Device as DfuDevice, DfuType, Error as DfuError};
use iced::alignment::Alignment::Center;
use iced::alignment::Horizontal;
use iced::widget::{column, container, row, Column, Space};
use iced::{time, Element, Length, Subscription, Task};
use futures::stream::{self, Stream, StreamExt};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use probe_rs::{MemoryInterface, Permissions, Core, Error as ProbeError};
use probe_rs::probe::DebugProbeInfo;
use probe_rs::probe::list::Lister;
use std::time::Duration;
use tokio::task::spawn_blocking;

use crate::analyse::Message as AnalyseMessage;
use crate::app::AppMessage;
use crate::create::Message as CreateMessage;
use crate::studio::RuntimeInfo;
use crate::style::{Style, Link};

use crate::internal_error;

const DEVICE_DETECTION_RETRY_SHORT: Duration = Duration::from_secs(5);
const DEVICE_DETECTION_RETRY_LONG: Duration = Duration::from_secs(5);
const PROBE_CORE_HALT_TIMEOUT: Duration = Duration::from_millis(100);

/// Sources of work
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Client {
    Analyse,
    Create,
}

impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Client::Analyse => write!(f, "Analyse"),
            Client::Create => write!(f, "Create"),
        }
    }
}

/// A wrapper for DebugProbeInfo for use in pick_lists.
/// We do this so we can use the probe_type() not the default DebugProbeInfo
/// Display impl in the pick list
#[derive(Debug, Clone, PartialEq)]
struct DebugProbeInfoWrapper(DebugProbeInfo);

impl std::fmt::Display for DebugProbeInfoWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Your custom display logic here
        write!(f, "{} ({:04X}:{:04X})", self.0.probe_type(), self.0.vendor_id, self.0.product_id)
    }
}

impl Into<DebugProbeInfoWrapper> for DebugProbeInfo {
    fn into(self) -> DebugProbeInfoWrapper {
        DebugProbeInfoWrapper(self)
    }
}

/// A USB device type
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum UsbDeviceType {
    /// An STM32 bootloader
    Ice(DfuDevice),
    /// An RP2350 bootloader
    Fire(DfuDevice),
}

impl std::fmt::Display for UsbDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsbDeviceType::Ice(d) => write!(f, "Ice USB ({})", d.info()),
            UsbDeviceType::Fire(d) => write!(f, "Fire USB ({})", d.info()),
        }
    }
}

impl UsbDeviceType {
    fn dfu_device(&self) -> &DfuDevice {
        match self {
            UsbDeviceType::Ice(d) => d,
            UsbDeviceType::Fire(d) => d,
        }
    }

    fn dfu_device_info(&self) -> &DfuDeviceInfo {
        match self {
            UsbDeviceType::Ice(d) => d.info(),
            UsbDeviceType::Fire(d) => d.info(),
        }
    }

    fn from_dfu(dfu_device: DfuDevice) -> Option<Self> {
        match (dfu_device.info().vid, dfu_device.info().pid) {
            (0x0483, 0xDF11) => Some(UsbDeviceType::Ice(dfu_device)),
            (0x2E8A, 0x0005) => Some(UsbDeviceType::Fire(dfu_device)),
            _ => None,
        }
    }
}

/// Messages for devices
#[derive(Debug, Clone)]
pub enum Message {
    DetectProbes,
    ProbesDetected(Vec<DebugProbeInfo>),
    SelectProbe(DebugProbeInfo),
    SelectUsbDevice(UsbDeviceType),
    SelectDevice(DeviceType),
    ReadDevice {
        client: Client,
        chip_id: String,
        address: u32,
        words: usize,
    },
    DetectUsbDevices,
    UsbDevicesDetected(Vec<UsbDeviceType>),
    FlashFirmware(Client, Vec<u8>),
    FlashFirmwareResult(Client, Result<(), String>),
    DeviceData(Client, Vec<u8>),
    ReadFailed(Client, String),
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::DetectProbes => write!(f, "DetectProbes"),
            Message::ProbesDetected(probes) => {
                let probes_str = probes
                    .iter()
                    .map(|p| p.identifier.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "ProbesDetected({probes_str})")
            }
            Message::SelectDevice(device) => write!(f, "SelectDevice({})", device),
            Message::SelectProbe(probe) => write!(f, "SelectProbe({})", probe),
            Message::SelectUsbDevice(usb_device) => write!(f, "SelectUsbDevice({})", usb_device),
            Message::ReadDevice {
                client,
                chip_id,
                address,
                words,
            } => {
                write!(
                    f,
                    "ReadDevice(cliient={client}, chip_id={}, address=0x{:X}, words={})",
                    chip_id, address, words
                )
            }
            Message::DetectUsbDevices => write!(f, "DetectUsbDevices"),
            Message::UsbDevicesDetected(devices) => {
                let devices_str = devices.iter()
                    .map(|d| format!("VID={:04X}, PID={:04X}", d.dfu_device_info().vid, d.dfu_device_info().pid))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "UsbDevicesDetected({})", devices_str)
            }
            Message::FlashFirmware(client, data) => {
                write!(f, "FlashFirmware(client={client}, data_len={})", data.len())
            }
            Message::FlashFirmwareResult(client, result) => {
                match result {
                    Ok(()) => write!(f, "FlashFirmwareResult(client={client}, Ok)"),
                    Err(e) => write!(f, "FlashFirmwareResult(client={client}, Err: {})", e),
                }
            }
            Message::DeviceData(client, data) => {
                write!(f, "DeviceData(client={client}, {} bytes)", data.len())
            }
            Message::ReadFailed(client, error) => {
                write!(f, "ReadFailed(client={client}, {})", error)
            }
        }
    }
}

/// Device state
#[derive(Debug, Clone)]
pub struct Device {
    selected: DeviceType,
    selected_probe: Option<DebugProbeInfo>,
    selected_usb_device: Option<UsbDeviceType>,
    probes: Vec<DebugProbeInfo>,
    usb_devices: Vec<UsbDeviceType>,
    operating: Option<Client>,
}

impl Default for Device {
    fn default() -> Self {
        Self {
            selected: DeviceType::None,
            selected_probe: None,
            selected_usb_device: None,
            probes: Vec::new(),
            usb_devices: Vec::new(),
            operating: None,
        }
    }
}

impl Device {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selected(&self) -> &DeviceType {
        &self.selected
    }

    pub fn is_busy(&self) -> bool {
        self.operating.is_some()
    }

    pub fn is_idle(&self) -> bool {
        self.operating.is_none()
    }

    pub fn update(&mut self, _runtime_info: &RuntimeInfo, message: Message) -> Task<AppMessage> {
        match message {
            Message::DetectProbes => {
                if self.is_idle() {
                    Task::future(Self::get_probe_list_async())
                } else {
                    trace!("Skipping probe detection while operating");
                    Task::none()
                }
            }
            Message::ProbesDetected(probes) => {
                self.probes_detected(probes);
                Task::none()
            }
            Message::SelectDevice(device) => {
                debug!("Selecting device: {}", device);
                self.select_device(device)
            },
            Message::SelectProbe(probe) => {
                debug!("Selecting probe: {}", probe);
                self.select_probe(probe)
            },
            Message::SelectUsbDevice(usb_device) => {
                debug!("Selecting USB device: {}", usb_device);
                self.select_usb_device(usb_device)
            },
            Message::DetectUsbDevices => {
                if self.is_idle() {
                    Task::future(Self::get_usb_device_list_async())
                } else {
                    trace!("Skipping USB device detection while operating");
                    Task::none()
                }
            }
            Message::UsbDevicesDetected(devices) => {
                self.usb_devices_detected(devices);
                Task::none()
            }
            Message::FlashFirmware(client, data) => {
                debug!("{client} Flashing firmware");
                self.operating = Some(client.clone());
                self.selected.flash(client, data)
            }
            Message::FlashFirmwareResult(client, result) => {
                debug!("{client} Firmware flash complete: {}", if result.is_ok() { "OK" } else { "Error" });
                // Force a device re-enumeration after flashing firmware
                self.operating = None;
                let msg = match client {
                    Client::Analyse => AnalyseMessage::FlashComplete(result).into(),
                    Client::Create => CreateMessage::FlashFirmwareResult(result).into(),
                };
                Task::future(Self::get_usb_device_list_async())
                    .chain(Task::done(msg))
            }
            Message::ReadDevice {
                client,
                chip_id,
                address,
                words,
            } => {
                debug!("{client} Reading device memory at 0x{:08X}, {} words", address, words);
                if client != Client::Analyse {
                    internal_error!("Device read requested by unsupported client: {}", client);
                    return Task::none()
                }
                self.operating = Some(client.clone());
                self.selected.read(client, &chip_id, address, words)
            }
            Message::DeviceData(client, data) => {
                debug!("{client} Received device data: {} bytes", data.len());
                assert_eq!(client, Client::Analyse);
                self.operating = None;
                Task::done(AnalyseMessage::DeviceData(data).into())
            }
            Message::ReadFailed(client, error) => {
                warn!("{client} Device read failed: {}", error);
                assert_eq!(client, Client::Analyse);
                self.operating = None;
                Task::done(AnalyseMessage::ReadFailed(error).into())
            }
        }
    }

    fn has_detected_probes(&self) -> bool {
        !self.probes.is_empty()
    }

    fn has_detected_usb_devices(&self) -> bool {
        !self.usb_devices.is_empty()
    }

    fn probes_detected(&mut self, probes: Vec<DebugProbeInfo>) {
        self.probes = probes.clone();

        if self.selected_probe.is_none() {
            if let Some(probe) = self.probes.first().cloned() {
                self.set_selected_probe(Some(probe));
            } else {
                trace!("No probes detected");
            }
        } else {
            // Check if selected probe is still connected
            let still_connected = self.probes.iter().any(|p| {
                if let Some(selected_probe) = &self.selected_probe {
                    *p == *selected_probe
                } else {
                    false
                }
            });

            if !still_connected {
                // Clear out and possibly select a new Probe
                if let Some(was_selected) = &self.selected_probe {
                    info!(
                        "Selected probe has been disconnected: {}, {}",
                        was_selected.identifier,
                        was_selected.serial_number.as_deref().unwrap_or("N/A")
                    );
                }

                // See if there's a probe to reconnect to
                if let Some(new_probe) = self.probes.first() {
                    self.set_selected_probe(Some(new_probe.clone()));
                } else {
                    debug!("No probes available to auto-select");
                    self.set_selected_probe(None);
                }
            }
        }

        // Finally, if there's no selected device, but there's a selected probe
        // device, select it
        self.check_selected();
    }

    fn usb_devices_detected(&mut self, devices: Vec<UsbDeviceType>) {
        self.usb_devices = devices;

        if self.selected_usb_device.is_none() {
            if let Some(usb_device) = self.usb_devices.first().cloned() {
                self.set_selected_usb_device(Some(usb_device));
            } else {
                trace!("No USB devices detected");
            }
        } else {
            // Check if selected USB device is still connected
            let still_connected = self.usb_devices.iter().any(|d| {
                if let Some(selected_usb) = &self.selected_usb_device {
                    *d == *selected_usb
                } else {
                    false
                }
            });

            if !still_connected {
                // Clear out and possibly select a new USB device
                if let Some(was_selected) = &self.selected_usb_device {
                    info!(
                        "Selected USB device has been disconnected: {}",
                        was_selected
                    );
                }

                // See if there's a device to reconnect to
                if let Some(new_usb_device) = self.usb_devices.first() {
                    self.set_selected_usb_device(Some(new_usb_device.clone()));
                } else {
                    debug!("No USB devices available to auto-select");
                    self.set_selected_usb_device(None);
                }
            }
        }

        // Finally, if there's no selected device, but there's a selected USB
        // device, select it
        self.check_selected();
    }

    fn set_selected_probe(&mut self, probe: Option<DebugProbeInfo>) {
        if let Some(probe) = probe {
            self.selected_probe = Some(probe.clone());
            info!(
                "Selected probe: {}, {}",
                probe.identifier,
                probe.serial_number.as_deref().unwrap_or("N/A")
            );
        } else {
            // Clear out probe
            self.selected_probe = None;
            debug!("Cleared selected probe");
            if self.selected.debug_probe().is_some() {
                self.clear_selected();
            }
        }
    }

    fn set_selected_usb_device(&mut self, usb_device: Option<UsbDeviceType>) {
        if let Some(usb_device) = usb_device {
            self.selected_usb_device = Some(usb_device.clone());
            info!("Selected USB device: {}", usb_device);
        } else {
            // Clear out USB device
            self.selected_usb_device = None;
            debug!("Cleared selected USB device");
            if self.selected.usb_device().is_some() {
                self.clear_selected();
            }
        }
        self.check_selected();
    }

    // Called after a specific device type has been cleared
    fn clear_selected(&mut self) {
        debug!("Cleared selected device");
        self.selected = DeviceType::None;

        // See if there's another type to auto-select
        self.check_selected();
    }

    fn check_selected(&mut self) {
        if self.selected.is_none() {
            // Prefer USB over debug probe
            let changed = if let Some(usb_device) = &self.selected_usb_device {
                self.selected = DeviceType::from_usb(usb_device.clone());
                true
            } else if let Some(probe) = &self.selected_probe {
                self.selected = DeviceType::from_debug_probe(probe.clone());
                true
            } else {
                false
            };

            if changed {
                info!("Auto-selected active device: {}", self.selected);
            }
        }
    }

    fn select_device(&mut self, device: DeviceType) -> Task<AppMessage> {
        self.selected = device;
        Task::none()
    }

    fn select_probe(&mut self, probe: DebugProbeInfo) -> Task<AppMessage> {
        self.selected_probe = Some(probe);
        Task::none()
    }

    fn select_usb_device(&mut self, usb_device: UsbDeviceType) -> Task<AppMessage> {
        self.selected_usb_device = Some(usb_device);
        Task::none()
    }

    /// At startup we want to check USB devices, then probe devices, so any
    /// present USB device gets selected in preference to probe ones.
    pub fn get_devices_startup() -> impl Stream<Item = AppMessage> {
        stream::once(Self::get_usb_device_list_async())
            .chain(stream::once(Self::get_probe_list_async()))
    }

    pub async fn get_probe_list_async() -> AppMessage {
        let probes = Lister::new().list_all();
        if !probes.is_empty() {
            // Need to send ourselves a message, as we can't modify
            // self in this async block
            Message::ProbesDetected(probes).into()
        } else {
            Message::ProbesDetected(Vec::new()).into()
        }
    }

    async fn get_usb_device_list_async() -> AppMessage {
        match DfuDevice::search(Some(DfuType::InternalFlash)) {
            Ok(devices) => {
                // Turn into UsbDeviceType
                let usb_devices: Vec<UsbDeviceType> = devices.into_iter().map(UsbDeviceType::from_dfu).filter_map(|d| d).collect();
                Message::UsbDevicesDetected(usb_devices).into()
            }
            Err(e) => {
                warn!("Failed to detect USB devices:\n  - {}", e);
                Message::UsbDevicesDetected(Vec::new()).into()
            }
        }
    }

    pub fn view(&self) -> Column<'_, AppMessage> {
        // Create the Probe and USB pick list labels
        let left_col = column![
            container(Style::text_small("Probe:")).height(Length::Fixed(25.0)).align_y(Center),
            container(Style::text_small("USB:")).height(Length::Fixed(25.0)).align_y(Center),
            container(Style::text_small("Use:")).height(Length::Fixed(30.0)).align_y(Center),
        ]
        .spacing(10)
        .align_x(Horizontal::Right);

        // Create the Probe pick list
        let probe_list: Element<'_, AppMessage> = if self.has_detected_probes() {
            let options = self.probes.clone().into_iter().map(DebugProbeInfoWrapper).collect::<Vec<_>>();
            Style::pick_list_small(options, self.selected_probe.clone().map(DebugProbeInfoWrapper), |p| {
                DeviceType::from_debug_probe(p.0.clone()).selected_message()
            })
            .into()
        } else {
            Style::text_body("Not detected")
                .color(Style::COLOUR_DARK_GOLD)
                .into()
        };
        let probe_list = container(probe_list)
            .height(Length::Fixed(25.0))
            .align_y(Center);

        // Create the USB device pick list
        let usb_device_list: Element<'_, AppMessage> = if !self.usb_devices.is_empty() {
            let options = self.usb_devices.as_slice();
            Style::pick_list_small(options, self.selected_usb_device.clone(), |d| {
                DeviceType::from_usb(d.clone()).selected_message()
            })
            .into()
        } else {
            Style::text_body("Not detected")
                .color(Style::COLOUR_DARK_GOLD)
                .into()
        };
        let usb_device_list = container(usb_device_list)
            .height(Length::Fixed(25.0))
            .align_y(Center);

        // Figure out how the Probe/USB buttons should work
        let highlight_probe_button = self.selected().debug_probe().is_some();
        let highlight_usb_button = self.selected().usb_device().is_some();
        let on_press_probe = if self.selected().debug_probe().is_none() && self.selected_probe.is_some() {
            Some(Message::SelectDevice(
                DeviceType::from_debug_probe(self.selected_probe.as_ref().unwrap().clone()),
            ).into())
        } else {
            None
        };
        let on_press_usb = if self.selected().usb_device().is_none() && self.selected_usb_device.is_some() {
            Some(Message::SelectDevice(
                DeviceType::from_usb(self.selected_usb_device.as_ref().unwrap().clone()),
            ).into())
        } else {
            None
        };

        // Create the buttons
        let probe_button = Style::text_button_small("Probe", on_press_probe, highlight_probe_button);
        let usb_button = Style::text_button_small("USB", on_press_usb, highlight_usb_button);
        let help_button = Style::text_button_small("Help", Some(AppMessage::Help(true)), true);
        let button_row = row![
            probe_button,
            usb_button,
            Space::with_width(Length::Fill),
            help_button,
        ].spacing(10)
            .align_y(Center);
        let button_row = container(button_row)
            .height(Length::Fixed(30.0))
            .align_y(Center);

        // Put the pick lists and buttons together into a column
        let right_col = column![
            probe_list,
            usb_device_list,
            button_row,
        ].spacing(10);

        // Create the row for everything
        let pick_list_row = row![
            left_col.width(Length::FillPortion(1)),
            right_col.width(Length::FillPortion(5))
        ]
            .spacing(10)
            .align_y(Center);


        column![
            pick_list_row,
        ]
            .spacing(20)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let check_probes_duration = if self.has_detected_probes() {
            DEVICE_DETECTION_RETRY_LONG
        } else {
            DEVICE_DETECTION_RETRY_SHORT
        };
        let check_usb_devices_duration = if self.has_detected_usb_devices() {
            DEVICE_DETECTION_RETRY_LONG
        } else {
            DEVICE_DETECTION_RETRY_SHORT
        };

        Subscription::batch([
            time::every(check_usb_devices_duration).map(|_| Message::DetectUsbDevices),
            time::every(check_probes_duration).map(|_| Message::DetectProbes),
        ])
    }

    pub fn help_overlay(&self) -> Element<'_, AppMessage> {
        let main_content = if cfg!(target_os = "windows") {
            self.help_content_win()
        } else if cfg!(target_os = "linux") {
            self.help_content_linux()
        } else if cfg!(target_os = "macos") {
            self.help_content_macos()
        } else {
            Style::text_body("No device help available for this platform").into()
        };

        let exit_button = row![
            Style::text_button("Exit", Some(AppMessage::Help(false)), true),
        ];

        column![
            Style::text_h2("Device Help").align_x(Horizontal::Center),
            Style::horiz_line(),
            main_content,
            exit_button,
        ]
        .align_x(Horizontal::Center)
        .spacing(20).into()
    }

    pub fn help_content_linux(&self) -> Element<'_, AppMessage> {
        let help_row_1 = row![
            Style::text_body("When installing from the official One ROM Studio .deb package, udev rules should be automatically set up to allow One ROM Studio to access debug probes and One ROM USB devices."),
            Space::with_width(Length::Fill),
        ];
        let help_row_2 = row![
            Style::text_body("If you have compiled One ROM Studio from source, or are using a different distribution method, you may need to set up udev rules manually."),
            Space::with_width(Length::Fill),
        ];
        let help_row_3 = row![
            Style::text_body("See "),
            Style::link("here", Style::FONT_SIZE_BODY, Link::LinuxUdev),
            Style::text_body(" for instructions."),
            Space::with_width(Length::Fill),
        ];
        let help_row_4 = row![
            Style::text_body("Also try reconnecting the device, restarting One ROM Studio, and rebooting your machine."),
            Space::with_width(Length::Fill),
        ];
        column![
            help_row_1,
            help_row_2,
            help_row_3,
            help_row_4,
        ]
        .spacing(20)
        .align_x(Horizontal::Center)
        .into()
    }

    pub fn help_content_macos(&self) -> Element<'_, AppMessage> {
        let help_row_1 = row![
            Style::text_body("There is no special USB device setup required on macOS to allow One ROM Studio to access your devices.  However, when plugging in devices you may need to choose 'Allow' so that your Mac can access them."),
            Space::with_width(Length::Fill),
        ];
        let help_row_2 = row![
            Style::text_body("If One ROM does detect a connected device, try reconnecting it, restarting One ROM Studio, and rebooting your Mac."),
            Space::with_width(Length::Fill),
        ];
        let help_row_3 = row![
            Style::text_body("If problems persist, please raise a "),
            Style::link("GitHub issue", Style::FONT_SIZE_BODY, Link::GitHubIssue),
            Style::text_body("."),
            Space::with_width(Length::Fill),
        ];
        column![
            help_row_1,
            help_row_2,
            help_row_3,
        ]
        .spacing(20)
        .align_x(Horizontal::Center)
        .into()
    }

    pub fn help_content_win(&self) -> Element<'_, AppMessage> {
        let help_row_1 = row![
            Style::text_body("If you have plugged in a One ROM USB and it has not been detected, you may need to install the WinUSB driver for it."),
            Space::with_width(Length::Fill),
        ];
        let help_row_2 = row![
            Style::text_body("See "),
            Style::link("here", Style::FONT_SIZE_BODY, Link::WinUsb),
            Style::text_body(" for instructions."),
            Space::with_width(Length::Fill),
        ];
        let help_row_3 = row![
            Style::text_body("Generic debug probes should be automatically detected when plugged in, although specific probes may need a custom driver."),
            Space::with_width(Length::Fill),
        ];
        let help_row_4 = row![
            Style::text_body("Try reconnecting the device, restarting One ROM Studio, and rebooting your PC."),
            Space::with_width(Length::Fill),
        ];
        let help_row_5 = row![
            Style::text_body("If problems persist, please raise a "),
            Style::link("GitHub issue", Style::FONT_SIZE_BODY, Link::GitHubIssue),
            Style::text_body("."),
            Space::with_width(Length::Fill),
        ];
        column![
            help_row_1,
            help_row_2,
            help_row_3,
            help_row_4,
            help_row_5,
        ]
        .spacing(20)
        .align_x(Horizontal::Center)
        .into()
    }

}

/// A type of a device
#[derive(Debug, Default, Clone, PartialEq)]
pub enum DeviceType {
    /// None
    #[default]
    None,
    /// A device connected via a debug probe
    DebugProbe(DebugProbeInfo),
    /// A device connected via USB
    Usb(UsbDeviceType),
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceType::DebugProbe(info) => write!(
                f,
                "{}, {}",
                info.identifier,
                info.serial_number.as_deref().unwrap_or("N/A")
            ),
            DeviceType::Usb(usb_type) => write!(f, "Usb({})", usb_type),
            DeviceType::None => write!(f, "None"),
        }
    }
}

impl DeviceType {
    fn debug_probe(&self) -> Option<DebugProbeInfo> {
        if let DeviceType::DebugProbe(info) = self {
            Some(info.clone())
        } else {
            None
        }
    }

    fn usb_device(&self) -> Option<UsbDeviceType> {
        if let DeviceType::Usb(usb_type) = self {
            Some(usb_type.clone())
        } else {
            None
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, DeviceType::None)
    }

    fn from_debug_probe(info: DebugProbeInfo) -> Self {
        DeviceType::DebugProbe(info)
    }

    fn from_usb(usb_type: UsbDeviceType) -> Self {
        DeviceType::Usb(usb_type)
    }

    fn selected_message(&self) -> AppMessage {
        match self {
            DeviceType::DebugProbe(info) => Message::SelectProbe(info.clone()).into(),
            DeviceType::Usb(usb_type) => Message::SelectUsbDevice(usb_type.clone()).into(),
            DeviceType::None => unreachable!(),
        }
    }

    pub fn read(&self, client: Client, chip_id: &str, address: u32, words: usize) -> Task<AppMessage> {
        match self {
            DeviceType::DebugProbe(probe) => Task::future(Self::read_debug_probe_async(
                client,
                probe.clone(),
                chip_id.to_string(),
                address,
                words,
            )),
            DeviceType::Usb(d) => Task::future(Self::read_usb_device_async(
                client,
                d.clone(),
                address,
                words,
            )),
            DeviceType::None => {
                let log = "Attempted to read from None device";
                internal_error!("{log}");
                Task::done(Message::ReadFailed(client, log.to_string()).into())
            }
        }
    }

    pub fn flash(&self, client: Client, data: Vec<u8>) -> Task<AppMessage> {
        match self {
            DeviceType::DebugProbe(probe) => Task::future(Self::flash_firmware_probe_async(
                client,
                probe.clone(),
                data,
            )),
            DeviceType::Usb(usb_device) => Task::future(Self::flash_firmware_usb_async(
                client,
                usb_device.clone(),
                data,
            )),
            DeviceType::None => {
                let log = "Attempted to flash to None device";
                internal_error!("{log}");
                Task::done(Message::FlashFirmwareResult(client, Err(log.to_string())).into())
            }
        }
    }

    pub async fn read_usb_device_async(
        client: Client,
        usb_device: UsbDeviceType,
        address: u32,
        words: usize,
    ) -> AppMessage {
        // Allocate buffer for the read
        let mut buf = vec![0u32; words];
        
        let dfu_device = usb_device.dfu_device().clone();
        
        // Run the blocking USB operation on a separate thread
        let result = spawn_blocking(move || -> Result<Vec<u32>, DfuError> {
            dfu_device.upload(address, &mut buf)?;
            Ok(buf)
        }).await;
        
        match result {
            Ok(Ok(data)) => {
                let bytes: Vec<u8> = data.iter().flat_map(|w| w.to_le_bytes()).collect();
                Message::DeviceData(client, bytes).into()
            }
            Ok(Err(e)) => {
                Message::ReadFailed(client, format!("DFU upload failed:\n  - {}", e)).into()
            }
            Err(e) => {
                Message::ReadFailed(client, format!("Task join failed:\n  - {}", e)).into()
            }
        }
    }

    pub async fn flash_firmware_usb_async(
        client: Client, 
        usb_device: UsbDeviceType,
        data: Vec<u8>,
    ) -> AppMessage {
        let dfu_device = usb_device.dfu_device().clone();

        // Convert vec<u8> to vec<u32>
        let data: Vec<u32> = data.chunks(4).map(|chunk| {
            let mut bytes = [0u8; 4];
            for (i, &b) in chunk.iter().enumerate() {
                bytes[i] = b;
            }
            u32::from_le_bytes(bytes)
        }).collect();

        // Run the blocking USB operation on a separate thread
        let result = spawn_blocking(move || -> Result<(), DfuError> {
            dfu_device.mass_erase()?;
            dfu_device.download(0x08000000, &data)?;
            Ok(())
        }).await;

        match result {
            Ok(Ok(())) => {
                debug!("Successfully flashed firmware using USB device {usb_device}");
                Message::FlashFirmwareResult(client, Ok(())).into()
            }
            Ok(Err(e)) => {
                let log = format!("Failed to flash firmware to One ROM using USB device {usb_device}: {e}");
                warn!("{log}");
                Message::FlashFirmwareResult(client, Err(log)).into()
            }
            Err(e) => {
                let log = format!("Failed to flash firmware to One ROM using USB device {usb_device}: {e}");
                error!("{log}");
                Message::FlashFirmwareResult(client, Err(log)).into()
            }
        }
    }   

    async fn read_debug_probe_async(
        client: Client,
        probe: DebugProbeInfo,
        chip_id: String,
        address: u32,
        words: usize,
    ) -> AppMessage {
        let result = spawn_blocking(move || {
            Self::probe_init_and_operate_on_core(probe, chip_id, true, |core| {
                let mut buf = vec![0u32; words];
                core.read_32(address as u64, &mut buf)?;
                let bytes: Vec<u8> = buf.iter().flat_map(|w| w.to_le_bytes()).collect();
                Ok(bytes)
            })
        }).await;

        match result {
            Ok(Ok(bytes)) => Message::DeviceData(client, bytes).into(),
            Ok(Err(e)) => {
                let log = format!("Failed to read {words} words of memory at {address:#010X}: {e}");
                warn!("{log}");
                Message::ReadFailed(client, log).into()
            }
            Err(e) => {
                let log = format!("Failed to read {words} words of memory at {address:#010X}: {e}");
                warn!("{log}");
                Message::ReadFailed(client, log).into()
            }
        }
    }

    async fn flash_firmware_probe_async(
        client: Client,
        probe: DebugProbeInfo,
        data: Vec<u8>,
    ) -> AppMessage {
        let result = spawn_blocking(move || {
            Self::probe_flash(probe, "STM32F411RETx".to_string(), 0x08000000, &data)
        }).await;

        match result {
            Ok(Ok(())) => Message::FlashFirmwareResult(client, Ok(())).into(),
            Ok(Err(e)) => {
                let log = format!("Failed to flash firmware: {e}");
                warn!("{log}");
                Message::FlashFirmwareResult(client, Err(log)).into()
            }
            Err(e) => {
                let log = format!("Failed to flash firmware: {e}");
                error!("{log}");
                Message::FlashFirmwareResult(client, Err(log)).into()
            }
        }
    }

    // Helper to open a probe, attach to a chip, halt core, and run a closure
    fn probe_init_and_operate_on_core<F, R>(
        probe: DebugProbeInfo, 
        chip_id: String, 
        halt_core: bool,
        f: F
    ) -> Result<R, ProbeError> 
    where
        F: FnOnce(&mut Core) -> Result<R, ProbeError>
    {
        let probe = probe.open()?;
        let probe_name = probe.get_name();
        let mut session = probe.attach(chip_id, Permissions::default())?;
        let mut core = session.core(0)?;

        if halt_core {
            debug!("Halting core using probe {}", probe_name);
            core.halt(PROBE_CORE_HALT_TIMEOUT)?;
        }

        f(&mut core)
    }

    // Helper to open a probe and session, and run a closure
    fn probe_flash(
        probe: DebugProbeInfo, 
        chip_id: String, 
        load_address: u32,
        data: &[u8],
    ) -> Result<(), String> 
    {
        let probe = probe.open()
            .map_err(|e| e.to_string())?;
        let probe_name = probe.get_name();
        debug!("Flashing firmware using probe {}", probe_name);
        
        let mut session = probe.attach(chip_id, Permissions::default())
            .map_err(|e| e.to_string())?;
        
        let mut loader = session.target().flash_loader();
        loader.add_data(load_address as u64, &data)
            .map_err(|e| e.to_string())?;

        loader.commit(&mut session, probe_rs::flashing::DownloadOptions::default())
            .map_err(|e| e.to_string())?;

        Ok(())
    }

}
