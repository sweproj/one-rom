// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! One ROM generation Builder objects and functions

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use onerom_config::fw::FirmwareProperties;
use onerom_config::rom::RomType;

use crate::image::{CsConfig, CsLogic, Rom, RomSet, RomSetType, SizeHandling};
use crate::meta::Metadata;
use crate::{Error, Result};

/// Main Builder object
///
/// Model is to create the builder from a JSON config, retrieve the list of
/// files that need to be loaded, call `add_file` for each file once loaded,
/// then call `build` to generate the metadata and ROM images.
///
/// # Example
/// ```no_run
/// use onerom_config::fw::{FirmwareProperties, FirmwareVersion, ServeAlg};
/// use onerom_config::hw::Board;
/// # use onerom_gen::Error;
/// use onerom_gen::builder::{Builder, FileData};
///
/// # fn fetch_file(url: &str) -> Result<Vec<u8>, Error> {
/// #     // Dummy implementation for doc test
/// #     Ok(vec![0u8; 8192])
/// # }
/// #
/// let json = r#"{
///     "version": 1,
///     "description": "Example ROM configuration",
///     "rom_sets": [{
///         "type": "single",
///         "roms": [{
///             "file": "http://example.com/kernal.bin",
///             "type": "2764",
///             "cs1": 0
///         }]
///     }]
/// }"#;
///
/// // Create builder from JSON
/// let mut builder = Builder::from_json(json)?;
///
/// // Get list of files to load
/// let file_specs = builder.file_specs();
///
/// // Load each file (fetch or read from disk)
/// for spec in file_specs {
///     let data = fetch_file(&spec.source)?; // Your implementation
///     
///     builder.add_file(FileData {
///         id: spec.id,
///         data,
///     })?;
/// }
///
/// // Build flash images
/// let props = FirmwareProperties::new(
///     FirmwareVersion::new(0, 5, 1, 0),
///     Board::Ice24UsbH,
///     ServeAlg::Default,
///     false,
/// );
///
/// let (metadata_buf, rom_images_buf) = builder.build(props)?;
/// // Buffers ready to flash at appropriate offsets
/// # Ok::<(), onerom_gen::Error>(())
/// ```
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Builder {
    config: Config,
    files: BTreeMap<usize, Vec<u8>>,
}

impl Builder {
    /// Create from JSON config
    pub fn from_json(json: &str) -> Result<Self> {
        let config: Config = serde_json::from_str(json).map_err(|e| Error::InvalidConfig {
            error: e.to_string(),
        })?;

        Self::validate_config(&config)?;

        Ok(Self {
            config,
            files: BTreeMap::new(),
        })
    }

    fn validate_config(config: &Config) -> Result<()> {
        // Validate version
        if config.version != 1 {
            return Err(Error::UnsupportedConfigVersion {
                version: config.version,
            });
        }

        // Validate each rom set has roms
        for set in config.rom_sets.iter() {
            if set.roms.is_empty() {
                return Err(Error::NoRoms);
            }

            if set.roms.len() > 1 {
                if set.set_type == RomSetType::Single {
                    return Err(Error::TooManyRoms {
                        expected: 1,
                        actual: set.roms.len(),
                    });
                }
            }

            for rom in set.roms.iter() {
                for line in rom.rom_type.control_lines() {
                    // Make sure relevant CS lines are specified
                    if line.name != "ce" && line.name != "oe" {
                        let cs = match line.name {
                            "cs1" => &rom.cs1,
                            "cs2" => &rom.cs2,
                            "cs3" => &rom.cs3,
                            _ => {
                                return Err(Error::InvalidConfig {
                                    error: format!("Unknown control line {}", line.name),
                                });
                            }
                        };
                        if cs.is_none() {
                            return Err(Error::MissingCsConfig { line: line.name });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get list of files that need to be loaded
    pub fn file_specs(&self) -> Vec<FileSpec> {
        let mut specs = Vec::new();
        let mut id = 0;

        for (rom_set_num, rom_set) in self.config.rom_sets.iter().enumerate() {
            for rom in &rom_set.roms {
                specs.push(FileSpec {
                    id,
                    description: rom.description.clone(),
                    source: rom.file.clone(),
                    extract: rom.extract.clone(),
                    size_handling: rom.size_handling.clone(),
                    rom_type: rom.rom_type.clone(),
                    rom_size: rom.rom_type.size_bytes(),
                    cs1: rom.cs1,
                    cs2: rom.cs2,
                    cs3: rom.cs3,
                    set_id: rom_set_num,
                    set_type: rom_set.set_type.clone(),
                    set_description: rom_set.description.clone(),
                });
                id += 1;
            }
        }

        specs
    }

    /// Add a loaded file - called multiple times, once for each file that
    /// has been loaded
    pub fn add_file(&mut self, file: FileData) -> Result<()> {
        // Check if already added
        if self.files.contains_key(&file.id) {
            return Err(Error::DuplicateFile { id: file.id });
        }

        // Validate id is in range
        let total_files = self.total_file_count();
        if file.id >= total_files {
            return Err(Error::InvalidFile {
                id: file.id,
                total: total_files,
            });
        }

        self.files.insert(file.id, file.data);
        Ok(())
    }

    fn total_file_count(&self) -> usize {
        self.config.rom_sets.iter().map(|set| set.roms.len()).sum()
    }

    /// Generate metadata and ROM images once all files loaded
    ///
    /// Returns (metadata, Rom images)
    pub fn build(&self, props: FirmwareProperties) -> Result<(Vec<u8>, Vec<u8>)> {
        // Check all files loaded
        for ii in 0..self.total_file_count() {
            if !self.files.contains_key(&ii) {
                return Err(Error::MissingFile { id: ii });
            }
        }

        // Build Rom and RomSet objects together
        let mut rom_sets = Vec::new();
        let mut rom_id = 0;

        for (set_id, rom_set_config) in self.config.rom_sets.iter().enumerate() {
            let mut set_roms = Vec::new();

            for rom_config in &rom_set_config.roms {
                let data = self.files.get(&rom_id).unwrap();

                let filename = if rom_config.extract.is_some() {
                    format!("{}|{}", rom_config.file, rom_config.extract.as_ref().unwrap())
                } else {
                    rom_config.file.clone()
                };

                let rom = Rom::from_raw_rom_image(
                    rom_id,
                    filename,
                    data,
                    vec![0u8; rom_config.rom_type.size_bytes()],
                    &rom_config.rom_type,
                    CsConfig::new(rom_config.cs1, rom_config.cs2, rom_config.cs3),
                    &rom_config.size_handling,
                )?;
                set_roms.push(rom);
                rom_id += 1;
            }

            let rom_set = RomSet::new(
                set_id,
                rom_set_config.set_type.clone(),
                props.serve_alg(),
                set_roms,
            )?;
            rom_sets.push(rom_set);
        }

        // Build Metadata
        let metadata = Metadata::new(props.board(), rom_sets, props.boot_logging());

        // Get buffer sizes
        let metadata_size = metadata.metadata_len();
        let rom_data_size: usize = metadata.rom_images_size();
        let set_count = metadata.total_set_count();

        // Allocate buffers
        let mut metadata_buf = vec![0u8; metadata_size];
        let mut rom_data_buf = vec![0u8; rom_data_size];
        let mut rom_data_ptrs = vec![0u32; set_count];

        // Write metadata
        metadata.write_all(&mut metadata_buf, &mut rom_data_ptrs)?;
        // Note rom_data_ptrs unused here - absolute flash addresses.

        // Write ROM data
        metadata.write_roms(&mut rom_data_buf)?;

        // Done - return the two buffers
        Ok((metadata_buf, rom_data_buf))
    }
}

/// Details about a file to be loaded by the caller
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileSpec {
    pub id: usize,
    pub description: Option<String>,
    pub source: String,
    pub extract: Option<String>,
    pub size_handling: SizeHandling,
    pub rom_type: RomType,
    pub rom_size: usize,
    pub cs1: Option<CsLogic>,
    pub cs2: Option<CsLogic>,
    pub cs3: Option<CsLogic>,
    pub set_id: usize,
    pub set_type: RomSetType,
    pub set_description: Option<String>,
}

/// File data loaded by the caller, passed back to the builder
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileData {
    pub id: usize,
    pub data: Vec<u8>,
}

/// Top level configuration structure, deserialized from JSON
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub version: u32,
    pub description: String,
    pub detail: Option<String>,
    pub rom_sets: Vec<RomSetConfig>,
    pub notes: Option<String>,
}

/// ROM Set configuration structure, deserialized from JSON
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RomSetConfig {
    #[serde(rename = "type")]
    pub set_type: RomSetType,
    pub description: Option<String>,
    pub roms: Vec<RomConfig>,
}

/// ROM configuration structure, deserialized from JSON
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RomConfig {
    pub file: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub rom_type: RomType,
    pub cs1: Option<CsLogic>,
    pub cs2: Option<CsLogic>,
    pub cs3: Option<CsLogic>,
    #[serde(default)]
    pub size_handling: SizeHandling,
    pub extract: Option<String>,
}
