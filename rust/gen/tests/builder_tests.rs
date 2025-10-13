// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! Tests for onerom-gen Builder
//!
//! Progressive validation of metadata and ROM image generation.
//!
//! # Test Plan
//!
//! ## Phase 1: Basic Structure Tests ✓ COMPLETE
//! - [x] Single ROM set, single ROM, no boot logging
//! - [x] Validate metadata header (magic, version, count)
//! - [x] Validate ROM set structure (data ptr, size, roms ptr, count, serve alg, multi-cs)
//! - [x] Validate ROM pointer array
//! - [x] Validate ROM info structure (rom type, cs1/cs2/cs3 states)
//! - [x] Validate pointer chain (header → rom set → rom array → rom info)
//!
//! ## Phase 2: Multiple ROM Sets ✓ COMPLETE
//! - [x] Multiple single ROM sets (2-3 sets)
//! - [x] Validate ROM set array is correct
//! - [x] Validate each set independently
//! - [x] Validate each ROM info independently
//!
//! ## Phase 3: CS Configuration Tests ✓ COMPLETE
//! - [x] 2332 with CS1 + CS2 (both active low)
//! - [x] 2332 with CS1 active low, CS2 active high
//! - [x] 2316 with CS1 + CS2 + CS3 (all active low)
//! - [x] 2316 with mixed active high/low states
//! - [x] Validate CS states stored correctly
//!
//! ## Phase 4: Boot Logging (Filenames) ✓ COMPLETE
//! - [x] Single ROM with boot_logging enabled
//! - [x] Validate ROM info structure is 8 bytes (not 4)
//! - [x] Validate filename pointer points within metadata
//! - [x] Validate null-terminated filename string
//! - [x] Multiple ROMs with boot_logging
//!
//! ## Phase 5: Size Handling ✓ COMPLETE
//! - [x] Exact size match (no size_handling needed)
//! - [x] Duplicate (smaller file, exact divisor)
//! - [x] Pad (smaller file)
//! - [x] Error cases (too large, wrong divisor, unnecessary size_handling)
//!
//! ## Phase 6: Multi-ROM Sets ✓ COMPLETE
//! - [x] Banked ROM sets
//! - [x] Multi ROM sets
//! - [x] Validate serve algorithm selection
//! - [x] Validate multi-CS state
//!
//! ## Phase 7: ROM Images Buffer ✓ COMPLETE
//! - [x] Validate buffer size matches expectations
//! - [x] Note: ROM image bytes are "mangled" (address/data transformations)
//! - [x] Use board pin maps to verify correctness
//! - [x] Test address mapping
//! - [x] Test data bit reordering
//!
//! ## Phase 8: Edge Cases ✓ COMPLETE
//! - [x] 32 ROM sets (stress test)
//! - [x] Minimum ROM size (2KB - 2316)
//! - [x] Missing CS config (should error)
//! - [x] Adding files out of order
//! - [x] Adding duplicate files (should error)
//! - [x] Missing files at build time (should error)

#[cfg(test)]
mod tests {
    use onerom_config::fw::{FirmwareProperties, FirmwareVersion, ServeAlg};
    use onerom_config::hw::Board;
    use onerom_gen::builder::{Builder, FileData};
    use onerom_gen::image::CsLogic;

    // ========================================================================
    // Constants from C headers
    // ========================================================================
    
    const HEADER_MAGIC: &[u8; 16] = b"ONEROM_METADATA\0";
    const HEADER_VERSION: u32 = 1;
    const METADATA_HEADER_LEN: usize = 256;
    const ROM_SET_METADATA_LEN: usize = 16;
    const ROM_INFO_METADATA_LEN: usize = 4;
    const ROM_INFO_METADATA_LEN_WITH_FILENAME: usize = 8;
    
    // Metadata starts at flash_base + 48KB
    const METADATA_FLASH_OFFSET: u32 = 49152;
    
    // ROM type C enum values (from Rom::rom_type_c_enum_val in image.rs)
    const ROM_TYPE_2316: u8 = 0;
    const ROM_TYPE_2332: u8 = 1;
    const ROM_TYPE_2364: u8 = 2;

    // ========================================================================
    // Helper: Parse Metadata Header
    // ========================================================================
    
    /// Represents the onerom_metadata_header_t C structure
    #[derive(Debug)]
    struct MetadataHeader {
        magic: [u8; 16],
        version: u32,
        rom_set_count: u8,
        rom_sets_ptr: u32,
    }

    impl MetadataHeader {
        /// Parse the metadata header from the start of the buffer
        fn parse(buf: &[u8]) -> Self {
            assert!(
                buf.len() >= METADATA_HEADER_LEN,
                "Buffer too small: {} bytes, need {}",
                buf.len(),
                METADATA_HEADER_LEN
            );
            
            // Magic: offset 0, 16 bytes
            let mut magic = [0u8; 16];
            magic.copy_from_slice(&buf[0..16]);
            
            // Version: offset 16, 4 bytes (u32 little-endian)
            let version = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
            
            // ROM set count: offset 20, 1 byte
            let rom_set_count = buf[20];
            
            // Padding: offset 21, 3 bytes (we skip these)
            
            // ROM sets pointer: offset 24, 4 bytes (u32 little-endian)
            let rom_sets_ptr = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
            
            Self {
                magic,
                version,
                rom_set_count,
                rom_sets_ptr,
            }
        }
        
        /// Validate the header has correct magic and version
        fn validate_basic(&self) {
            assert_eq!(
                &self.magic,
                HEADER_MAGIC,
                "Magic bytes mismatch. Expected {:?}, got {:?}",
                HEADER_MAGIC,
                &self.magic
            );
            
            assert_eq!(
                self.version,
                HEADER_VERSION,
                "Version mismatch. Expected {}, got {}",
                HEADER_VERSION,
                self.version
            );
            
            assert!(
                self.rom_set_count > 0,
                "ROM set count must be > 0, got {}",
                self.rom_set_count
            );
        }
    }

    // ========================================================================
    // Helper: Parse ROM Set Structure
    // ========================================================================
    
    /// Represents the sdrr_rom_set_t C structure
    #[derive(Debug)]
    struct RomSetStruct {
        data_ptr: u32,
        size: u32,
        roms_ptr: u32,
        rom_count: u8,
        serve_alg: u8,
        multi_cs_state: u8,
    }

    impl RomSetStruct {
        /// Parse ROM set structure from buffer at given offset
        fn parse(buf: &[u8], offset: usize) -> Self {
            assert!(
                buf.len() >= offset + ROM_SET_METADATA_LEN,
                "Buffer too small: {} bytes, need {} at offset {}",
                buf.len(),
                offset + ROM_SET_METADATA_LEN,
                offset
            );
            
            // Data pointer: offset + 0, 4 bytes
            let data_ptr = u32::from_le_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ]);
            
            // Size: offset + 4, 4 bytes
            let size = u32::from_le_bytes([
                buf[offset + 4],
                buf[offset + 5],
                buf[offset + 6],
                buf[offset + 7],
            ]);
            
            // ROMs pointer: offset + 8, 4 bytes
            let roms_ptr = u32::from_le_bytes([
                buf[offset + 8],
                buf[offset + 9],
                buf[offset + 10],
                buf[offset + 11],
            ]);
            
            // ROM count: offset + 12, 1 byte
            let rom_count = buf[offset + 12];
            
            // Serve algorithm: offset + 13, 1 byte
            let serve_alg = buf[offset + 13];
            
            // Multi-CS state: offset + 14, 1 byte
            let multi_cs_state = buf[offset + 14];
            
            // Padding at offset + 15 (1 byte) - ignored
            
            Self {
                data_ptr,
                size,
                roms_ptr,
                rom_count,
                serve_alg,
                multi_cs_state,
            }
        }
    }

    // ========================================================================
    // Helper: Parse ROM Info Structure
    // ========================================================================
    
    /// Represents the sdrr_rom_info_t C structure
    #[derive(Debug)]
    struct RomInfoStruct {
        rom_type: u8,
        cs1_state: u8,
        cs2_state: u8,
        cs3_state: u8,
        filename_ptr: Option<u32>,
    }

    impl RomInfoStruct {
        /// Parse ROM info structure from buffer at given offset (without filename)
        fn parse(buf: &[u8], offset: usize) -> Self {
            assert!(
                buf.len() >= offset + ROM_INFO_METADATA_LEN,
                "Buffer too small: {} bytes, need {} at offset {}",
                buf.len(),
                offset + ROM_INFO_METADATA_LEN,
                offset
            );
            
            let rom_type = buf[offset];
            let cs1_state = buf[offset + 1];
            let cs2_state = buf[offset + 2];
            let cs3_state = buf[offset + 3];
            
            Self {
                rom_type,
                cs1_state,
                cs2_state,
                cs3_state,
                filename_ptr: None,
            }
        }
        
        /// Parse ROM info structure from buffer at given offset (with filename)
        fn parse_with_filename(buf: &[u8], offset: usize) -> Self {
            assert!(
                buf.len() >= offset + ROM_INFO_METADATA_LEN_WITH_FILENAME,
                "Buffer too small: {} bytes, need {} at offset {}",
                buf.len(),
                offset + ROM_INFO_METADATA_LEN_WITH_FILENAME,
                offset
            );
            
            let rom_type = buf[offset];
            let cs1_state = buf[offset + 1];
            let cs2_state = buf[offset + 2];
            let cs3_state = buf[offset + 3];
            
            let filename_ptr = u32::from_le_bytes([
                buf[offset + 4],
                buf[offset + 5],
                buf[offset + 6],
                buf[offset + 7],
            ]);
            
            Self {
                rom_type,
                cs1_state,
                cs2_state,
                cs3_state,
                filename_ptr: Some(filename_ptr),
            }
        }
    }

    // ========================================================================
    // Helper: Create test firmware properties
    // ========================================================================
    
    fn default_fw_props() -> FirmwareProperties {
        FirmwareProperties::new(
            FirmwareVersion::new(0, 5, 1, 0),
            Board::Ice24UsbH,
            ServeAlg::Default,
            false, // boot_logging disabled
        )
    }
    
    fn fw_props_with_logging() -> FirmwareProperties {
        FirmwareProperties::new(
            FirmwareVersion::new(0, 5, 1, 0),
            Board::Ice24UsbH,
            ServeAlg::Default,
            true, // boot_logging enabled
        )
    }

    // ========================================================================
    // Helper: Parse null-terminated string
    // ========================================================================
    
    fn parse_null_terminated_string(buf: &[u8], offset: usize) -> String {
        let mut end = offset;
        while end < buf.len() && buf[end] != 0 {
            end += 1;
        }
        
        assert!(
            end < buf.len(),
            "No null terminator found starting at offset {}",
            offset
        );
        
        String::from_utf8_lossy(&buf[offset..end]).to_string()
    }

    // ========================================================================
    // Helper: Create test ROM data
    // ========================================================================
    
    fn create_test_rom_data(size: usize, fill_byte: u8) -> Vec<u8> {
        vec![fill_byte; size]
    }

    // ========================================================================
    // TEST 1: Simplest possible - single ROM set, single ROM
    // ========================================================================
    
    #[test]
    fn test_phase1_single_rom_basic() {
        // Minimal JSON config: single ROM set with one 2364 ROM (8KB)
        let json = r#"{
            "version": 1,
            "description": "Phase 1 basic test",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        // Parse the JSON
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Get the file specs - should be exactly 1
        let file_specs = builder.file_specs();
        assert_eq!(file_specs.len(), 1, "Should have exactly 1 file");
        assert_eq!(file_specs[0].id, 0, "File ID should be 0");
        assert_eq!(file_specs[0].source, "test.rom", "File source should match");
        
        // Create 8KB of test data (2364 is 8KB)
        let rom_data = create_test_rom_data(8192, 0xAA);
        
        // Add the file
        builder.add_file(FileData {
            id: 0,
            data: rom_data,
        }).expect("Failed to add file");
        
        // Build the metadata and ROM images
        let props = default_fw_props();
        let (metadata_buf, rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Basic sanity checks
        assert!(
            !metadata_buf.is_empty(),
            "Metadata buffer should not be empty"
        );
        assert!(
            metadata_buf.len() >= METADATA_HEADER_LEN,
            "Metadata buffer should be at least {} bytes, got {}",
            METADATA_HEADER_LEN,
            metadata_buf.len()
        );
        assert!(
            !rom_images_buf.is_empty(),
            "ROM images buffer should not be empty"
        );
        
        // Parse and validate the metadata header
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        
        // Check ROM set count
        assert_eq!(
            header.rom_set_count, 1,
            "Should have exactly 1 ROM set"
        );
        
        println!("✓ Phase 1 Test 1: Basic single ROM set passed");
        println!("  - Metadata size: {} bytes", metadata_buf.len());
        println!("  - ROM images size: {} bytes", rom_images_buf.len());
        println!("  - ROM set count: {}", header.rom_set_count);
    }

    // ========================================================================
    // TEST 2: Validate ROM Set Structure
    // ========================================================================
    
    #[test]
    fn test_phase1_rom_set_structure() {
        let json = r#"{
            "version": 1,
            "description": "Phase 1 ROM set structure test",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        let rom_data = create_test_rom_data(8192, 0xAA);
        builder.add_file(FileData { id: 0, data: rom_data }).expect("Failed to add file");
        
        let props = default_fw_props();
        let board = props.board();
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        
        // Calculate where ROM set structure should be
        // rom_sets_ptr is an absolute flash address, need to convert to metadata buffer offset
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let rom_set_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        
        // Validate offset is within metadata buffer
        assert!(
            rom_set_offset < metadata_buf.len(),
            "ROM set pointer {} (offset {}) outside metadata buffer (size {})",
            header.rom_sets_ptr,
            rom_set_offset,
            metadata_buf.len()
        );
        
        // Parse the ROM set structure
        let rom_set = RomSetStruct::parse(&metadata_buf, rom_set_offset);
        
        // Validate data pointer (should be flash_base + 64KB)
        let expected_data_ptr = flash_base + 65536;
        assert_eq!(
            rom_set.data_ptr, expected_data_ptr,
            "Data pointer mismatch. Expected 0x{:08X}, got 0x{:08X}",
            expected_data_ptr, rom_set.data_ptr
        );
        
        // Validate size (STM32F4 single ROM = 16KB)
        let expected_size = 16384u32;
        assert_eq!(
            rom_set.size, expected_size,
            "Size mismatch. Expected {} bytes, got {} bytes",
            expected_size, rom_set.size
        );
        
        // Validate ROMs pointer is within metadata
        let roms_ptr_offset = (rom_set.roms_ptr - metadata_flash_start) as usize;
        assert!(
            roms_ptr_offset < metadata_buf.len(),
            "ROMs pointer {} (offset {}) outside metadata buffer (size {})",
            rom_set.roms_ptr,
            roms_ptr_offset,
            metadata_buf.len()
        );
        
        // Validate ROM count
        assert_eq!(
            rom_set.rom_count, 1,
            "ROM count should be 1, got {}",
            rom_set.rom_count
        );
        
        // Validate serve algorithm (single ROM uses AddrOnCs)
        let expected_serve_alg = ServeAlg::AddrOnCs.c_enum_value();
        assert_eq!(
            rom_set.serve_alg, expected_serve_alg,
            "Serve algorithm mismatch. Expected {} (AddrOnCs), got {}",
            expected_serve_alg, rom_set.serve_alg
        );
        
        // Validate multi-CS state (single ROM should be Ignore)
        let expected_multi_cs = CsLogic::Ignore.c_enum_val();
        assert_eq!(
            rom_set.multi_cs_state, expected_multi_cs,
            "Multi-CS state mismatch. Expected {} (Ignore), got {}",
            expected_multi_cs, rom_set.multi_cs_state
        );
        
        println!("✓ Phase 1 Test 2: ROM set structure validation passed");
        println!("  - Data pointer: 0x{:08X}", rom_set.data_ptr);
        println!("  - Size: {} bytes", rom_set.size);
        println!("  - ROMs pointer: 0x{:08X}", rom_set.roms_ptr);
        println!("  - ROM count: {}", rom_set.rom_count);
        println!("  - Serve algorithm: {}", rom_set.serve_alg);
        println!("  - Multi-CS state: {}", rom_set.multi_cs_state);
    }

    // ========================================================================
    // TEST 3: Validate ROM Info Structure
    // ========================================================================
    
    #[test]
    fn test_phase1_rom_info_structure() {
        let json = r#"{
            "version": 1,
            "description": "Phase 1 ROM info structure test",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        let rom_data = create_test_rom_data(8192, 0xAA);
        builder.add_file(FileData { id: 0, data: rom_data }).expect("Failed to add file");
        
        let props = default_fw_props();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header and ROM set
        let header = MetadataHeader::parse(&metadata_buf);
        let rom_set_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set = RomSetStruct::parse(&metadata_buf, rom_set_offset);
        
        // Parse ROM pointer array to get pointer to first ROM info
        let rom_array_offset = (rom_set.roms_ptr - metadata_flash_start) as usize;
        assert!(
            rom_array_offset + 4 <= metadata_buf.len(),
            "ROM array pointer {} (offset {}) outside metadata buffer",
            rom_set.roms_ptr,
            rom_array_offset
        );
        
        // Read the first pointer from the ROM pointer array (4 bytes)
        let rom_info_ptr = u32::from_le_bytes([
            metadata_buf[rom_array_offset],
            metadata_buf[rom_array_offset + 1],
            metadata_buf[rom_array_offset + 2],
            metadata_buf[rom_array_offset + 3],
        ]);
        
        // Convert to buffer offset
        let rom_info_offset = (rom_info_ptr - metadata_flash_start) as usize;
        assert!(
            rom_info_offset < metadata_buf.len(),
            "ROM info pointer {} (offset {}) outside metadata buffer",
            rom_info_ptr,
            rom_info_offset
        );
        
        // Parse the ROM info structure
        let rom_info = RomInfoStruct::parse(&metadata_buf, rom_info_offset);
        
        // Validate ROM type (2364 = 2)
        assert_eq!(
            rom_info.rom_type, ROM_TYPE_2364,
            "ROM type mismatch. Expected {} (2364), got {}",
            ROM_TYPE_2364, rom_info.rom_type
        );
        
        // Validate CS1 state (active_low = 0)
        let expected_cs1 = CsLogic::ActiveLow.c_enum_val();
        assert_eq!(
            rom_info.cs1_state, expected_cs1,
            "CS1 state mismatch. Expected {} (ActiveLow), got {}",
            expected_cs1, rom_info.cs1_state
        );
        
        // Validate CS2 state (not used for 2364, should be CS_NOT_USED = 2)
        let expected_cs2 = CsLogic::Ignore.c_enum_val();
        assert_eq!(
            rom_info.cs2_state, expected_cs2,
            "CS2 state mismatch. Expected {} (Ignore), got {}",
            expected_cs2, rom_info.cs2_state
        );
        
        // Validate CS3 state (not used for 2364, should be CS_NOT_USED = 2)
        let expected_cs3 = CsLogic::Ignore.c_enum_val();
        assert_eq!(
            rom_info.cs3_state, expected_cs3,
            "CS3 state mismatch. Expected {} (Ignore), got {}",
            expected_cs3, rom_info.cs3_state
        );
        
        println!("✓ Phase 1 Test 3: ROM info structure validation passed");
        println!("  - ROM type: {} (2364)", rom_info.rom_type);
        println!("  - CS1 state: {} (ActiveLow)", rom_info.cs1_state);
        println!("  - CS2 state: {} (Ignore)", rom_info.cs2_state);
        println!("  - CS3 state: {} (Ignore)", rom_info.cs3_state);
    }

    // ========================================================================
    // PHASE 2: Multiple ROM Sets
    // ========================================================================

    // ========================================================================
    // TEST 5: Two ROM Sets
    // ========================================================================
    
    #[test]
    fn test_phase2_two_rom_sets() {
        let json = r#"{
            "version": 1,
            "description": "Phase 2 two ROM sets test",
            "rom_sets": [
                {
                    "type": "single",
                    "description": "Set 0 - 2364",
                    "roms": [{
                        "file": "set0.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    }]
                },
                {
                    "type": "single",
                    "description": "Set 1 - 2332",
                    "roms": [{
                        "file": "set1.rom",
                        "type": "2332",
                        "cs1": "active_low",
                        "cs2": "active_high"
                    }]
                }
            ]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Add ROM data for both sets
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA), // 2364 = 8KB
        }).expect("Failed to add file 0");
        
        builder.add_file(FileData {
            id: 1,
            data: create_test_rom_data(4096, 0x55), // 2332 = 4KB
        }).expect("Failed to add file 1");
        
        let props = default_fw_props();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        
        // Validate we have 2 ROM sets
        assert_eq!(
            header.rom_set_count, 2,
            "Should have 2 ROM sets, got {}",
            header.rom_set_count
        );
        
        // Parse both ROM sets
        let rom_set0_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set0 = RomSetStruct::parse(&metadata_buf, rom_set0_offset);
        
        let rom_set1_offset = rom_set0_offset + ROM_SET_METADATA_LEN;
        let rom_set1 = RomSetStruct::parse(&metadata_buf, rom_set1_offset);
        
        // Validate Set 0 (2364)
        assert_eq!(rom_set0.rom_count, 1, "Set 0 should have 1 ROM");
        assert_eq!(rom_set0.size, 16384, "Set 0 size should be 16KB");
        assert_eq!(
            rom_set0.serve_alg,
            ServeAlg::AddrOnCs.c_enum_value(),
            "Set 0 serve algorithm mismatch"
        );
        
        // Validate Set 1 (2332)
        assert_eq!(rom_set1.rom_count, 1, "Set 1 should have 1 ROM");
        assert_eq!(rom_set1.size, 16384, "Set 1 size should be 16KB");
        assert_eq!(
            rom_set1.serve_alg,
            ServeAlg::AddrOnCs.c_enum_value(),
            "Set 1 serve algorithm mismatch"
        );
        
        // Validate Set 0 data pointer (flash_base + 64KB)
        let expected_data_ptr0 = flash_base + 65536;
        assert_eq!(
            rom_set0.data_ptr, expected_data_ptr0,
            "Set 0 data pointer mismatch"
        );
        
        // Validate Set 1 data pointer (flash_base + 64KB + 16KB)
        let expected_data_ptr1 = flash_base + 65536 + 16384;
        assert_eq!(
            rom_set1.data_ptr, expected_data_ptr1,
            "Set 1 data pointer mismatch"
        );
        
        // Parse ROM info for Set 0
        let rom_array0_offset = (rom_set0.roms_ptr - metadata_flash_start) as usize;
        let rom_info0_ptr = u32::from_le_bytes([
            metadata_buf[rom_array0_offset],
            metadata_buf[rom_array0_offset + 1],
            metadata_buf[rom_array0_offset + 2],
            metadata_buf[rom_array0_offset + 3],
        ]);
        let rom_info0_offset = (rom_info0_ptr - metadata_flash_start) as usize;
        let rom_info0 = RomInfoStruct::parse(&metadata_buf, rom_info0_offset);
        
        // Validate Set 0 ROM info
        assert_eq!(rom_info0.rom_type, ROM_TYPE_2364, "Set 0 ROM type mismatch");
        assert_eq!(rom_info0.cs1_state, CsLogic::ActiveLow.c_enum_val());
        assert_eq!(rom_info0.cs2_state, CsLogic::Ignore.c_enum_val());
        assert_eq!(rom_info0.cs3_state, CsLogic::Ignore.c_enum_val());
        
        // Parse ROM info for Set 1
        let rom_array1_offset = (rom_set1.roms_ptr - metadata_flash_start) as usize;
        let rom_info1_ptr = u32::from_le_bytes([
            metadata_buf[rom_array1_offset],
            metadata_buf[rom_array1_offset + 1],
            metadata_buf[rom_array1_offset + 2],
            metadata_buf[rom_array1_offset + 3],
        ]);
        let rom_info1_offset = (rom_info1_ptr - metadata_flash_start) as usize;
        let rom_info1 = RomInfoStruct::parse(&metadata_buf, rom_info1_offset);
        
        // Validate Set 1 ROM info
        assert_eq!(rom_info1.rom_type, ROM_TYPE_2332, "Set 1 ROM type mismatch");
        assert_eq!(rom_info1.cs1_state, CsLogic::ActiveLow.c_enum_val());
        assert_eq!(rom_info1.cs2_state, CsLogic::ActiveHigh.c_enum_val());
        assert_eq!(rom_info1.cs3_state, CsLogic::Ignore.c_enum_val());
        
        println!("✓ Phase 2 Test 1: Two ROM sets validation passed");
        println!("  Set 0:");
        println!("    - ROM type: {} (2364)", rom_info0.rom_type);
        println!("    - Data pointer: 0x{:08X}", rom_set0.data_ptr);
        println!("    - Size: {} bytes", rom_set0.size);
        println!("    - CS1: {} (ActiveLow)", rom_info0.cs1_state);
        println!("  Set 1:");
        println!("    - ROM type: {} (2332)", rom_info1.rom_type);
        println!("    - Data pointer: 0x{:08X}", rom_set1.data_ptr);
        println!("    - Size: {} bytes", rom_set1.size);
        println!("    - CS1: {} (ActiveLow), CS2: {} (ActiveHigh)", 
                 rom_info1.cs1_state, rom_info1.cs2_state);
    }

    // ========================================================================
    // TEST 6: Three ROM Sets
    // ========================================================================
    
    #[test]
    fn test_phase2_three_rom_sets() {
        let json = r#"{
            "version": 1,
            "description": "Phase 2 three ROM sets test",
            "rom_sets": [
                {
                    "type": "single",
                    "roms": [{
                        "file": "set0.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    }]
                },
                {
                    "type": "single",
                    "roms": [{
                        "file": "set1.rom",
                        "type": "2332",
                        "cs1": "active_low",
                        "cs2": "active_high"
                    }]
                },
                {
                    "type": "single",
                    "roms": [{
                        "file": "set2.rom",
                        "type": "2316",
                        "cs1": "active_low",
                        "cs2": "active_low",
                        "cs3": "active_low"
                    }]
                }
            ]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Add ROM data for all three sets
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA), // 2364 = 8KB
        }).expect("Failed to add file 0");
        
        builder.add_file(FileData {
            id: 1,
            data: create_test_rom_data(4096, 0x55), // 2332 = 4KB
        }).expect("Failed to add file 1");
        
        builder.add_file(FileData {
            id: 2,
            data: create_test_rom_data(2048, 0xFF), // 2316 = 2KB
        }).expect("Failed to add file 2");
        
        let props = default_fw_props();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        
        // Validate we have 3 ROM sets
        assert_eq!(
            header.rom_set_count, 3,
            "Should have 3 ROM sets, got {}",
            header.rom_set_count
        );
        
        // Parse all three ROM sets
        let rom_set0_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set0 = RomSetStruct::parse(&metadata_buf, rom_set0_offset);
        
        let rom_set1_offset = rom_set0_offset + ROM_SET_METADATA_LEN;
        let rom_set1 = RomSetStruct::parse(&metadata_buf, rom_set1_offset);
        
        let rom_set2_offset = rom_set1_offset + ROM_SET_METADATA_LEN;
        let rom_set2 = RomSetStruct::parse(&metadata_buf, rom_set2_offset);
        
        // Validate data pointers are sequential
        let expected_data_ptr0 = flash_base + 65536;
        let expected_data_ptr1 = expected_data_ptr0 + 16384;
        let expected_data_ptr2 = expected_data_ptr1 + 16384;
        
        assert_eq!(rom_set0.data_ptr, expected_data_ptr0, "Set 0 data pointer");
        assert_eq!(rom_set1.data_ptr, expected_data_ptr1, "Set 1 data pointer");
        assert_eq!(rom_set2.data_ptr, expected_data_ptr2, "Set 2 data pointer");
        
        // Parse and validate ROM info for Set 0 (2364)
        let rom_array0_offset = (rom_set0.roms_ptr - metadata_flash_start) as usize;
        let rom_info0_ptr = u32::from_le_bytes([
            metadata_buf[rom_array0_offset],
            metadata_buf[rom_array0_offset + 1],
            metadata_buf[rom_array0_offset + 2],
            metadata_buf[rom_array0_offset + 3],
        ]);
        let rom_info0 = RomInfoStruct::parse(&metadata_buf, (rom_info0_ptr - metadata_flash_start) as usize);
        
        assert_eq!(rom_info0.rom_type, ROM_TYPE_2364);
        assert_eq!(rom_info0.cs1_state, CsLogic::ActiveLow.c_enum_val());
        
        // Parse and validate ROM info for Set 1 (2332)
        let rom_array1_offset = (rom_set1.roms_ptr - metadata_flash_start) as usize;
        let rom_info1_ptr = u32::from_le_bytes([
            metadata_buf[rom_array1_offset],
            metadata_buf[rom_array1_offset + 1],
            metadata_buf[rom_array1_offset + 2],
            metadata_buf[rom_array1_offset + 3],
        ]);
        let rom_info1 = RomInfoStruct::parse(&metadata_buf, (rom_info1_ptr - metadata_flash_start) as usize);
        
        assert_eq!(rom_info1.rom_type, ROM_TYPE_2332);
        assert_eq!(rom_info1.cs1_state, CsLogic::ActiveLow.c_enum_val());
        assert_eq!(rom_info1.cs2_state, CsLogic::ActiveHigh.c_enum_val());
        
        // Parse and validate ROM info for Set 2 (2316)
        let rom_array2_offset = (rom_set2.roms_ptr - metadata_flash_start) as usize;
        let rom_info2_ptr = u32::from_le_bytes([
            metadata_buf[rom_array2_offset],
            metadata_buf[rom_array2_offset + 1],
            metadata_buf[rom_array2_offset + 2],
            metadata_buf[rom_array2_offset + 3],
        ]);
        let rom_info2 = RomInfoStruct::parse(&metadata_buf, (rom_info2_ptr - metadata_flash_start) as usize);
        
        assert_eq!(rom_info2.rom_type, ROM_TYPE_2316);
        assert_eq!(rom_info2.cs1_state, CsLogic::ActiveLow.c_enum_val());
        assert_eq!(rom_info2.cs2_state, CsLogic::ActiveLow.c_enum_val());
        assert_eq!(rom_info2.cs3_state, CsLogic::ActiveLow.c_enum_val());
        
        println!("✓ Phase 2 Test 2: Three ROM sets validation passed");
        println!("  Set 0: 2364, CS1=Low");
        println!("  Set 1: 2332, CS1=Low, CS2=High");
        println!("  Set 2: 2316, CS1=Low, CS2=Low, CS3=Low");
        println!("  Data pointers: 0x{:08X}, 0x{:08X}, 0x{:08X}", 
                 rom_set0.data_ptr, rom_set1.data_ptr, rom_set2.data_ptr);
    }

    // ========================================================================
    // PHASE 4: Boot Logging (Filenames)
    // ========================================================================

    // ========================================================================
    // TEST 4: Validate ROM Info with Filename
    // ========================================================================
    
    #[test]
    fn test_phase4_boot_logging_filename() {
        let json = r#"{
            "version": 1,
            "description": "Phase 4 boot logging test",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test_filename.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        let rom_data = create_test_rom_data(8192, 0xAA);
        builder.add_file(FileData { id: 0, data: rom_data }).expect("Failed to add file");
        
        let props = fw_props_with_logging();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header and ROM set
        let header = MetadataHeader::parse(&metadata_buf);
        let rom_set_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set = RomSetStruct::parse(&metadata_buf, rom_set_offset);
        
        // Parse ROM pointer array to get pointer to first ROM info
        let rom_array_offset = (rom_set.roms_ptr - metadata_flash_start) as usize;
        let rom_info_ptr = u32::from_le_bytes([
            metadata_buf[rom_array_offset],
            metadata_buf[rom_array_offset + 1],
            metadata_buf[rom_array_offset + 2],
            metadata_buf[rom_array_offset + 3],
        ]);
        
        // Convert to buffer offset
        let rom_info_offset = (rom_info_ptr - metadata_flash_start) as usize;
        
        // Parse the ROM info structure WITH filename
        let rom_info = RomInfoStruct::parse_with_filename(&metadata_buf, rom_info_offset);
        
        // Validate basic ROM info fields (same as Phase 1)
        assert_eq!(rom_info.rom_type, ROM_TYPE_2364);
        assert_eq!(rom_info.cs1_state, CsLogic::ActiveLow.c_enum_val());
        assert_eq!(rom_info.cs2_state, CsLogic::Ignore.c_enum_val());
        assert_eq!(rom_info.cs3_state, CsLogic::Ignore.c_enum_val());
        
        // Validate filename pointer exists
        assert!(
            rom_info.filename_ptr.is_some(),
            "Filename pointer should be present with boot_logging enabled"
        );
        
        let filename_ptr = rom_info.filename_ptr.unwrap();
        
        // Validate filename pointer is within metadata buffer
        let filename_offset = (filename_ptr - metadata_flash_start) as usize;
        assert!(
            filename_offset < metadata_buf.len(),
            "Filename pointer {} (offset {}) outside metadata buffer (size {})",
            filename_ptr,
            filename_offset,
            metadata_buf.len()
        );
        
        // Parse the null-terminated filename string
        let filename = parse_null_terminated_string(&metadata_buf, filename_offset);
        
        // Validate filename matches what we specified in JSON
        assert_eq!(
            filename, "test_filename.rom",
            "Filename mismatch. Expected 'test_filename.rom', got '{}'",
            filename
        );
        
        println!("✓ Phase 4 Test 1: Boot logging with filename passed");
        println!("  - ROM type: {} (2364)", rom_info.rom_type);
        println!("  - CS states: {}, {}, {}", rom_info.cs1_state, rom_info.cs2_state, rom_info.cs3_state);
        println!("  - Filename pointer: 0x{:08X}", filename_ptr);
        println!("  - Filename: '{}'", filename);
    }

    // ========================================================================
    // TEST 8: Multiple ROMs with Boot Logging
    // ========================================================================
    
    #[test]
    fn test_phase4_multiple_roms_with_boot_logging() {
        let json = r#"{
            "version": 1,
            "description": "Phase 4 multiple ROMs with boot logging test",
            "rom_sets": [
                {
                    "type": "single",
                    "roms": [{
                        "file": "first_rom.bin",
                        "type": "2364",
                        "cs1": "active_low"
                    }]
                },
                {
                    "type": "single",
                    "roms": [{
                        "file": "second_rom.bin",
                        "type": "2332",
                        "cs1": "active_low",
                        "cs2": "active_high"
                    }]
                }
            ]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("Failed to add file 0");
        
        builder.add_file(FileData {
            id: 1,
            data: create_test_rom_data(4096, 0x55),
        }).expect("Failed to add file 1");
        
        let props = fw_props_with_logging();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header
        let header = MetadataHeader::parse(&metadata_buf);
        assert_eq!(header.rom_set_count, 2);
        
        // Parse both ROM sets
        let rom_set0_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set0 = RomSetStruct::parse(&metadata_buf, rom_set0_offset);
        
        let rom_set1_offset = rom_set0_offset + ROM_SET_METADATA_LEN;
        let rom_set1 = RomSetStruct::parse(&metadata_buf, rom_set1_offset);
        
        // Parse ROM info for Set 0 with filename
        let rom_array0_offset = (rom_set0.roms_ptr - metadata_flash_start) as usize;
        let rom_info0_ptr = u32::from_le_bytes([
            metadata_buf[rom_array0_offset],
            metadata_buf[rom_array0_offset + 1],
            metadata_buf[rom_array0_offset + 2],
            metadata_buf[rom_array0_offset + 3],
        ]);
        let rom_info0_offset = (rom_info0_ptr - metadata_flash_start) as usize;
        let rom_info0 = RomInfoStruct::parse_with_filename(&metadata_buf, rom_info0_offset);
        
        // Validate Set 0 ROM info
        assert_eq!(rom_info0.rom_type, ROM_TYPE_2364);
        assert!(rom_info0.filename_ptr.is_some(), "Set 0 should have filename");
        
        let filename0_ptr = rom_info0.filename_ptr.unwrap();
        let filename0_offset = (filename0_ptr - metadata_flash_start) as usize;
        assert!(filename0_offset < metadata_buf.len(), "Set 0 filename pointer out of bounds");
        
        let filename0 = parse_null_terminated_string(&metadata_buf, filename0_offset);
        assert_eq!(filename0, "first_rom.bin", "Set 0 filename mismatch");
        
        // Parse ROM info for Set 1 with filename
        let rom_array1_offset = (rom_set1.roms_ptr - metadata_flash_start) as usize;
        let rom_info1_ptr = u32::from_le_bytes([
            metadata_buf[rom_array1_offset],
            metadata_buf[rom_array1_offset + 1],
            metadata_buf[rom_array1_offset + 2],
            metadata_buf[rom_array1_offset + 3],
        ]);
        let rom_info1_offset = (rom_info1_ptr - metadata_flash_start) as usize;
        let rom_info1 = RomInfoStruct::parse_with_filename(&metadata_buf, rom_info1_offset);
        
        // Validate Set 1 ROM info
        assert_eq!(rom_info1.rom_type, ROM_TYPE_2332);
        assert!(rom_info1.filename_ptr.is_some(), "Set 1 should have filename");
        
        let filename1_ptr = rom_info1.filename_ptr.unwrap();
        let filename1_offset = (filename1_ptr - metadata_flash_start) as usize;
        assert!(filename1_offset < metadata_buf.len(), "Set 1 filename pointer out of bounds");
        
        let filename1 = parse_null_terminated_string(&metadata_buf, filename1_offset);
        assert_eq!(filename1, "second_rom.bin", "Set 1 filename mismatch");
        
        // Validate filenames are at different locations
        assert_ne!(
            filename0_offset, filename1_offset,
            "Filenames should be at different offsets"
        );
        
        println!("✓ Phase 4 Test 2: Multiple ROMs with boot logging passed");
        println!("  Set 0: '{}' at offset {}", filename0, filename0_offset);
        println!("  Set 1: '{}' at offset {}", filename1, filename1_offset);
    }

    // ========================================================================
    // PHASE 3: CS Configuration Tests
    // ========================================================================

    // ========================================================================
    // TEST 9: 2332 with CS1 and CS2 Both Active Low
    // ========================================================================
    
    #[test]
    fn test_phase3_2332_both_cs_active_low() {
        let json = r#"{
            "version": 1,
            "description": "Phase 3 2332 with both CS active low",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2332",
                    "cs1": "active_low",
                    "cs2": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(4096, 0xAA), // 2332 = 4KB
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header and ROM set
        let header = MetadataHeader::parse(&metadata_buf);
        let rom_set_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set = RomSetStruct::parse(&metadata_buf, rom_set_offset);
        
        // Parse ROM info
        let rom_array_offset = (rom_set.roms_ptr - metadata_flash_start) as usize;
        let rom_info_ptr = u32::from_le_bytes([
            metadata_buf[rom_array_offset],
            metadata_buf[rom_array_offset + 1],
            metadata_buf[rom_array_offset + 2],
            metadata_buf[rom_array_offset + 3],
        ]);
        let rom_info_offset = (rom_info_ptr - metadata_flash_start) as usize;
        let rom_info = RomInfoStruct::parse(&metadata_buf, rom_info_offset);
        
        // Validate ROM type
        assert_eq!(rom_info.rom_type, ROM_TYPE_2332, "ROM type should be 2332");
        
        // Validate CS1 is active low
        assert_eq!(
            rom_info.cs1_state,
            CsLogic::ActiveLow.c_enum_val(),
            "CS1 should be active low"
        );
        
        // Validate CS2 is active low
        assert_eq!(
            rom_info.cs2_state,
            CsLogic::ActiveLow.c_enum_val(),
            "CS2 should be active low"
        );
        
        // CS3 should be ignored for 2332
        assert_eq!(
            rom_info.cs3_state,
            CsLogic::Ignore.c_enum_val(),
            "CS3 should be ignored for 2332"
        );
        
        println!("✓ Phase 3 Test 1: 2332 with both CS active low passed");
        println!("  - ROM type: {} (2332)", rom_info.rom_type);
        println!("  - CS1: {} (ActiveLow)", rom_info.cs1_state);
        println!("  - CS2: {} (ActiveLow)", rom_info.cs2_state);
        println!("  - CS3: {} (Ignore)", rom_info.cs3_state);
    }

    // ========================================================================
    // TEST 10: 2316 with Mixed CS States
    // ========================================================================
    
    #[test]
    fn test_phase3_2316_mixed_cs_states() {
        let json = r#"{
            "version": 1,
            "description": "Phase 3 2316 with mixed CS states",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2316",
                    "cs1": "active_low",
                    "cs2": "active_high",
                    "cs3": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(2048, 0xAA), // 2316 = 2KB
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header and ROM set
        let header = MetadataHeader::parse(&metadata_buf);
        let rom_set_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set = RomSetStruct::parse(&metadata_buf, rom_set_offset);
        
        // Parse ROM info
        let rom_array_offset = (rom_set.roms_ptr - metadata_flash_start) as usize;
        let rom_info_ptr = u32::from_le_bytes([
            metadata_buf[rom_array_offset],
            metadata_buf[rom_array_offset + 1],
            metadata_buf[rom_array_offset + 2],
            metadata_buf[rom_array_offset + 3],
        ]);
        let rom_info_offset = (rom_info_ptr - metadata_flash_start) as usize;
        let rom_info = RomInfoStruct::parse(&metadata_buf, rom_info_offset);
        
        // Validate ROM type
        assert_eq!(rom_info.rom_type, ROM_TYPE_2316, "ROM type should be 2316");
        
        // Validate CS1 is active low
        assert_eq!(
            rom_info.cs1_state,
            CsLogic::ActiveLow.c_enum_val(),
            "CS1 should be active low"
        );
        
        // Validate CS2 is active high
        assert_eq!(
            rom_info.cs2_state,
            CsLogic::ActiveHigh.c_enum_val(),
            "CS2 should be active high"
        );
        
        // Validate CS3 is active low
        assert_eq!(
            rom_info.cs3_state,
            CsLogic::ActiveLow.c_enum_val(),
            "CS3 should be active low"
        );
        
        println!("✓ Phase 3 Test 2: 2316 with mixed CS states passed");
        println!("  - ROM type: {} (2316)", rom_info.rom_type);
        println!("  - CS1: {} (ActiveLow)", rom_info.cs1_state);
        println!("  - CS2: {} (ActiveHigh)", rom_info.cs2_state);
        println!("  - CS3: {} (ActiveLow)", rom_info.cs3_state);
    }

    // ========================================================================
    // PHASE 5: Size Handling
    // ========================================================================

    // ========================================================================
    // TEST 11: Exact Size Match
    // ========================================================================
    
    #[test]
    fn test_phase5_exact_size_match() {
        let json = r#"{
            "version": 1,
            "description": "Phase 5 exact size match",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create exactly 8KB for 2364
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build should succeed with exact size");
        
        // Basic validation
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 1);
        
        println!("✓ Phase 5 Test 1: Exact size match passed");
        println!("  - 8KB file for 2364 (8KB ROM) - no size_handling needed");
    }

    // ========================================================================
    // TEST 12: Duplicate Size Handling
    // ========================================================================
    
    #[test]
    fn test_phase5_duplicate_size_handling() {
        let json = r#"{
            "version": 1,
            "description": "Phase 5 duplicate size handling",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low",
                    "size_handling": "duplicate"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create 4KB file for 8KB ROM (exact divisor)
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(4096, 0xAA),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build should succeed with duplicate");
        
        // Basic validation
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 1);
        
        println!("✓ Phase 5 Test 2: Duplicate size handling passed");
        println!("  - 4KB file duplicated to fill 8KB ROM");
    }

    // ========================================================================
    // TEST 13: Pad Size Handling
    // ========================================================================
    
    #[test]
    fn test_phase5_pad_size_handling() {
        let json = r#"{
            "version": 1,
            "description": "Phase 5 pad size handling",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low",
                    "size_handling": "pad"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create 3KB file for 8KB ROM (not an exact divisor, must pad)
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(3072, 0x55),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build should succeed with pad");
        
        // Basic validation
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 1);
        
        println!("✓ Phase 5 Test 3: Pad size handling passed");
        println!("  - 3KB file padded to fill 8KB ROM");
    }

    // ========================================================================
    // TEST 14: Error - File Too Large
    // ========================================================================
    
    #[test]
    fn test_phase5_error_file_too_large() {
        let json = r#"{
            "version": 1,
            "description": "Phase 5 error - file too large",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create 10KB file for 8KB ROM - too large
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(10240, 0xAA),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let result = builder.build(props);
        
        // Should fail
        assert!(result.is_err(), "Build should fail with file too large");
        
        println!("✓ Phase 5 Test 4: Error - file too large correctly rejected");
    }

    // ========================================================================
    // TEST 15: Error - Wrong Divisor for Duplicate
    // ========================================================================
    
    #[test]
    fn test_phase5_error_wrong_divisor() {
        let json = r#"{
            "version": 1,
            "description": "Phase 5 error - wrong divisor",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low",
                    "size_handling": "duplicate"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create 3KB file for 8KB ROM with duplicate - 3KB is not exact divisor of 8KB
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(3072, 0xAA),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let result = builder.build(props);
        
        // Should fail
        assert!(result.is_err(), "Build should fail with non-divisor size for duplicate");
        
        println!("✓ Phase 5 Test 5: Error - wrong divisor for duplicate correctly rejected");
    }

    // ========================================================================
    // TEST 16: Error - Unnecessary size_handling
    // ========================================================================
    
    #[test]
    fn test_phase5_error_unnecessary_size_handling() {
        let json = r#"{
            "version": 1,
            "description": "Phase 5 error - unnecessary size_handling",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low",
                    "size_handling": "pad"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create exactly 8KB file but specified pad - unnecessary
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let result = builder.build(props);
        
        // Should fail
        assert!(result.is_err(), "Build should fail with unnecessary size_handling");
        
        println!("✓ Phase 5 Test 6: Error - unnecessary size_handling correctly rejected");
    }

    // ========================================================================
    // PHASE 6: Multi-ROM Sets
    // ========================================================================

    // ========================================================================
    // TEST 17: Banked ROM Set (2 ROMs)
    // ========================================================================
    
    #[test]
    fn test_phase6_banked_rom_set() {
        let json = r#"{
            "version": 1,
            "description": "Phase 6 banked ROM set",
            "rom_sets": [{
                "type": "banked",
                "roms": [
                    {
                        "file": "bank0.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    },
                    {
                        "file": "bank1.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    }
                ]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("Failed to add file 0");
        
        builder.add_file(FileData {
            id: 1,
            data: create_test_rom_data(8192, 0x55),
        }).expect("Failed to add file 1");
        
        let props = default_fw_props();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 1, "Should have 1 ROM set");
        
        // Parse ROM set
        let rom_set_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set = RomSetStruct::parse(&metadata_buf, rom_set_offset);
        
        // Validate ROM count
        assert_eq!(rom_set.rom_count, 2, "Banked set should have 2 ROMs");
        
        // Validate size - banked sets use 64KB
        assert_eq!(rom_set.size, 65536, "Banked set size should be 64KB");
        
        // Validate serve algorithm
        assert_eq!(
            rom_set.serve_alg,
            ServeAlg::AddrOnCs.c_enum_value(),
            "Banked set should use AddrOnCs serve algorithm"
        );
        
        // Validate multi-CS state (should be active_low since both ROMs use it)
        assert_eq!(
            rom_set.multi_cs_state,
            CsLogic::ActiveLow.c_enum_val(),
            "Multi-CS state should be ActiveLow"
        );
        
        println!("✓ Phase 6 Test 1: Banked ROM set passed");
        println!("  - 2 ROMs in banked set");
        println!("  - Size: {} bytes", rom_set.size);
        println!("  - Serve algorithm: {}", rom_set.serve_alg);
        println!("  - Multi-CS state: {}", rom_set.multi_cs_state);
    }

    // ========================================================================
    // TEST 18: Multi ROM Set (2 ROMs)
    // ========================================================================
    
    #[test]
    fn test_phase6_multi_rom_set() {
        let json = r#"{
            "version": 1,
            "description": "Phase 6 multi ROM set",
            "rom_sets": [{
                "type": "multi",
                "roms": [
                    {
                        "file": "rom0.bin",
                        "type": "2364",
                        "cs1": "active_low",
                        "cs2": "ignore",
                        "cs3": "ignore"
                    },
                    {
                        "file": "rom1.bin",
                        "type": "2364",
                        "cs1": "active_low",
                        "cs2": "ignore",
                        "cs3": "ignore"
                    }
                ]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("Failed to add file 0");
        
        builder.add_file(FileData {
            id: 1,
            data: create_test_rom_data(8192, 0x55),
        }).expect("Failed to add file 1");
        
        let props = default_fw_props();
        let board = props.board();
        let flash_base = board.mcu_family().get_flash_base();
        let metadata_flash_start = flash_base + METADATA_FLASH_OFFSET;
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Parse metadata header
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 1, "Should have 1 ROM set");
        
        // Parse ROM set
        let rom_set_offset = (header.rom_sets_ptr - metadata_flash_start) as usize;
        let rom_set = RomSetStruct::parse(&metadata_buf, rom_set_offset);
        
        // Validate ROM count
        assert_eq!(rom_set.rom_count, 2, "Multi set should have 2 ROMs");
        
        // Validate size - multi sets use 64KB
        assert_eq!(rom_set.size, 65536, "Multi set size should be 64KB");
        
        // Validate serve algorithm - multi sets use AddrOnAnyCs
        assert_eq!(
            rom_set.serve_alg,
            ServeAlg::AddrOnAnyCs.c_enum_value(),
            "Multi set should use AddrOnAnyCs serve algorithm"
        );
        
        // Validate multi-CS state
        assert_eq!(
            rom_set.multi_cs_state,
            CsLogic::ActiveLow.c_enum_val(),
            "Multi-CS state should be ActiveLow"
        );
        
        println!("✓ Phase 6 Test 2: Multi ROM set passed");
        println!("  - 2 ROMs in multi set");
        println!("  - Size: {} bytes", rom_set.size);
        println!("  - Serve algorithm: {} (AddrOnAnyCs)", rom_set.serve_alg);
        println!("  - Multi-CS state: {}", rom_set.multi_cs_state);
    }

    // ========================================================================
    // PHASE 8: Edge Cases
    // ========================================================================

    // ========================================================================
    // TEST 19: Error - Adding Duplicate Files
    // ========================================================================
    
    #[test]
    fn test_phase8_error_duplicate_files() {
        let json = r#"{
            "version": 1,
            "description": "Phase 8 error - duplicate files",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Add file once
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("First add should succeed");
        
        // Try to add same file again
        let result = builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xBB),
        });
        
        // Should fail
        assert!(result.is_err(), "Adding duplicate file should fail");
        
        println!("✓ Phase 8 Test 1: Error - duplicate files correctly rejected");
    }

    // ========================================================================
    // TEST 20: Error - Missing Files at Build Time
    // ========================================================================
    
    #[test]
    fn test_phase8_error_missing_files() {
        let json = r#"{
            "version": 1,
            "description": "Phase 8 error - missing files",
            "rom_sets": [
                {
                    "type": "single",
                    "roms": [{
                        "file": "test0.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    }]
                },
                {
                    "type": "single",
                    "roms": [{
                        "file": "test1.rom",
                        "type": "2332",
                        "cs1": "active_low",
                        "cs2": "active_high"
                    }]
                }
            ]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Add only first file, skip second
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("Adding file 0 should succeed");
        
        // Try to build without adding file 1
        let props = default_fw_props();
        let result = builder.build(props);
        
        // Should fail
        assert!(result.is_err(), "Building with missing files should fail");
        
        println!("✓ Phase 8 Test 2: Error - missing files at build time correctly rejected");
    }

    // ========================================================================
    // TEST 21: Adding Files Out of Order
    // ========================================================================
    
    #[test]
    fn test_phase8_files_out_of_order() {
        let json = r#"{
            "version": 1,
            "description": "Phase 8 files out of order",
            "rom_sets": [
                {
                    "type": "single",
                    "roms": [{
                        "file": "test0.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    }]
                },
                {
                    "type": "single",
                    "roms": [{
                        "file": "test1.rom",
                        "type": "2332",
                        "cs1": "active_low",
                        "cs2": "active_high"
                    }]
                },
                {
                    "type": "single",
                    "roms": [{
                        "file": "test2.rom",
                        "type": "2316",
                        "cs1": "active_low",
                        "cs2": "active_low",
                        "cs3": "active_low"
                    }]
                }
            ]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Add files out of order: 2, 0, 1
        builder.add_file(FileData {
            id: 2,
            data: create_test_rom_data(2048, 0xFF),
        }).expect("Adding file 2 should succeed");
        
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(8192, 0xAA),
        }).expect("Adding file 0 should succeed");
        
        builder.add_file(FileData {
            id: 1,
            data: create_test_rom_data(4096, 0x55),
        }).expect("Adding file 1 should succeed");
        
        // Build should succeed even with files added out of order
        let props = default_fw_props();
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build should succeed");
        
        // Basic validation
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 3);
        
        println!("✓ Phase 8 Test 3: Files added out of order correctly handled");
        println!("  - Added files in order: 2, 0, 1");
        println!("  - Build succeeded with 3 ROM sets");
    }

    // ========================================================================
    // TEST 22: Error - Missing CS Config
    // ========================================================================
    
    #[test]
    fn test_phase8_error_missing_cs_config() {
        // 2332 requires CS2 to be specified
        let json = r#"{
            "version": 1,
            "description": "Phase 8 error - missing CS2 for 2332",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2332",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        // Should fail at JSON parsing/validation
        let result = Builder::from_json(json);
        
        assert!(result.is_err(), "Missing CS2 for 2332 should fail");
        
        println!("✓ Phase 8 Test 4: Error - missing CS config correctly rejected");
    }

    // ========================================================================
    // TEST 23: Minimum ROM Size (2KB - 2316)
    // ========================================================================
    
    #[test]
    fn test_phase8_minimum_rom_size() {
        let json = r#"{
            "version": 1,
            "description": "Phase 8 minimum ROM size test",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2316",
                    "cs1": "active_low",
                    "cs2": "active_low",
                    "cs3": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        builder.add_file(FileData {
            id: 0,
            data: create_test_rom_data(2048, 0xAA), // 2316 = 2KB
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build should succeed");
        
        // Basic validation
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 1);
        
        println!("✓ Phase 8 Test 5: Minimum ROM size (2KB) handled correctly");
    }

    // ========================================================================
    // TEST 24: 32 ROM Sets (Stress Test)
    // ========================================================================
    
    #[test]
    fn test_phase8_32_rom_sets() {
        // Build JSON for 32 ROM sets
        let mut json = String::from(r#"{
            "version": 1,
            "description": "Phase 8 32 ROM sets stress test",
            "rom_sets": ["#);
        
        for i in 0..32 {
            if i > 0 {
                json.push_str(",");
            }
            json.push_str(&format!(r#"
                {{
                    "type": "single",
                    "roms": [{{
                        "file": "test{}.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    }}]
                }}"#, i));
        }
        
        json.push_str(r#"
            ]
        }"#);
        
        let mut builder = Builder::from_json(&json).expect("Failed to parse JSON");
        
        // Add all 32 files
        for i in 0..32 {
            builder.add_file(FileData {
                id: i,
                data: create_test_rom_data(8192, (i as u8).wrapping_mul(8)),
            }).expect(&format!("Failed to add file {}", i));
        }
        
        let props = default_fw_props();
        let (metadata_buf, _rom_images_buf) = builder.build(props).expect("Build should succeed");
        
        // Validate
        let header = MetadataHeader::parse(&metadata_buf);
        header.validate_basic();
        assert_eq!(header.rom_set_count, 32, "Should have 32 ROM sets");
        
        println!("✓ Phase 8 Test 6: 32 ROM sets stress test passed");
        println!("  - Successfully built metadata for 32 ROM sets");
    }

    // ========================================================================
    // PHASE 7: ROM Images Buffer
    // ========================================================================

    // Helper: Transform logical address to physical address based on board pin mapping
    fn logical_to_physical_address(logical_addr: usize, board: onerom_config::hw::Board) -> usize {
        let addr_pins = board.addr_pins();
        let mut physical_address = 0;
        
        // For each address line, if the bit is set in logical address,
        // set the corresponding physical pin bit
        for (addr_line, &phys_pin) in addr_pins.iter().enumerate() {
            if logical_addr & (1 << addr_line) != 0 {
                let pin = phys_pin as usize;
                // Handle boards where address pins are shifted
                let bit_position = if pin >= 8 && addr_pins.iter().all(|&p| p >= 8 || p < 8) {
                    // All pins either <8 or >=8, use adjusted position
                    if addr_pins[0] >= 8 { pin - 8 } else { pin }
                } else {
                    pin
                };
                physical_address |= 1 << bit_position;
            }
        }
        
        physical_address
    }

    // Helper: Transform logical data byte to physical byte based on board pin mapping
    fn logical_to_physical_byte(logical_byte: u8, board: onerom_config::hw::Board) -> u8 {
        let data_pins = board.data_pins();
        let mut physical_byte = 0;
        
        // For each data line, if the bit is set in logical byte,
        // set the corresponding physical pin bit
        for (data_line, &phys_pin) in data_pins.iter().enumerate() {
            if logical_byte & (1 << data_line) != 0 {
                physical_byte |= 1 << (phys_pin % 8);
            }
        }
        
        physical_byte
    }

    // ========================================================================
    // TEST 25: ROM Images Buffer Validation
    // ========================================================================
    
    #[test]
    fn test_phase7_rom_images_buffer() {
        let json = r#"{
            "version": 1,
            "description": "Phase 7 ROM images buffer test",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "test.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create test data with predictable pattern: data[addr] = addr as u8
        let rom_size = 8192; // 2364 = 8KB
        let mut test_data = Vec::with_capacity(rom_size);
        for addr in 0..rom_size {
            test_data.push(addr as u8);
        }
        
        builder.add_file(FileData {
            id: 0,
            data: test_data.clone(),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let board = props.board();
        let (_metadata_buf, rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Validate every byte in the ROM images buffer
        let mut errors = 0;
        let max_errors_to_report = 10;
        
        for logical_addr in 0..rom_size {
            let logical_byte = test_data[logical_addr];
            
            // Transform to physical address and byte
            let physical_addr = logical_to_physical_address(logical_addr, board);
            let physical_byte = logical_to_physical_byte(logical_byte, board);
            
            // Check ROM images buffer
            let actual_byte = rom_images_buf[physical_addr];
            
            if actual_byte != physical_byte {
                errors += 1;
                if errors <= max_errors_to_report {
                    println!(
                        "  Mismatch at logical_addr=0x{:04X} (physical=0x{:04X}): expected 0x{:02X}, got 0x{:02X}",
                        logical_addr, physical_addr, physical_byte, actual_byte
                    );
                }
            }
        }
        
        if errors > max_errors_to_report {
            println!("  ... and {} more errors", errors - max_errors_to_report);
        }
        
        assert_eq!(errors, 0, "Found {} byte mismatches in ROM images buffer", errors);
        
        println!("✓ Phase 7 Test 1: ROM images buffer validation passed");
        println!("  - Verified all {} bytes with address/data transformations", rom_size);
    }
    
    // Helper: Unscramble physical byte to logical byte based on board pin mapping
    fn unscramble_physical_byte(physical_byte: u8, board: onerom_config::hw::Board) -> u8 {
        let data_pins = board.data_pins();
        let mut logical_byte = 0;
        
        // For each physical pin, if the bit is set, set the corresponding logical data line bit
        for (data_line, &phys_pin) in data_pins.iter().enumerate() {
            if physical_byte & (1 << (phys_pin % 8)) != 0 {
                logical_byte |= 1 << data_line;
            }
        }
        
        logical_byte
    }

    // Helper: Read byte from ROM images buffer using logical address
    // (simulates what firmware does - reverse the transformations)
    fn read_rom_byte(
        rom_images_buf: &[u8],
        logical_addr: usize,
        board: onerom_config::hw::Board,
    ) -> u8 {
        // Transform logical address to physical address
        let physical_addr = logical_to_physical_address(logical_addr, board);
        
        // Read the physical byte
        let physical_byte = rom_images_buf[physical_addr];
        
        // Reverse transform physical byte to logical byte
        unscramble_physical_byte(physical_byte, board)
    }

    // Helper Read bye from ROM images buffer using absolute address
    fn read_rom_byte_abs(
        rom_images_buf: &[u8],
        abs_addr: usize,
        board: onerom_config::hw::Board,
    ) -> u8 {
        let physical_byte = rom_images_buf[abs_addr];
        unscramble_physical_byte(physical_byte, board)
    }

    // ========================================================================
    // TEST 26: ROM Images Buffer with Random Data
    // ========================================================================
    
    #[test]
    fn test_phase7_rom_images_random_data() {
        let json = r#"{
            "version": 1,
            "description": "Phase 7 ROM images with random data",
            "rom_sets": [{
                "type": "single",
                "roms": [{
                    "file": "random.rom",
                    "type": "2364",
                    "cs1": "active_low"
                }]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create random test data using a simple PRNG for reproducibility
        let rom_size = 8192; // 2364 = 8KB
        let mut test_data = Vec::with_capacity(rom_size);
        let mut seed = 0x12345678u32;
        for _ in 0..rom_size {
            // Simple linear congruential generator
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            test_data.push((seed >> 24) as u8);
        }
        
        builder.add_file(FileData {
            id: 0,
            data: test_data.clone(),
        }).expect("Failed to add file");
        
        let props = default_fw_props();
        let board = props.board();
        let (_metadata_buf, rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Read back every byte using logical addresses and verify against original
        let mut errors = 0;
        let max_errors_to_report = 10;
        
        for logical_addr in 0..rom_size {
            let expected_byte = test_data[logical_addr];
            let actual_byte = read_rom_byte(&rom_images_buf, logical_addr, board);
            
            if actual_byte != expected_byte {
                errors += 1;
                if errors <= max_errors_to_report {
                    println!(
                        "  Mismatch at logical_addr=0x{:04X}: expected 0x{:02X}, got 0x{:02X}",
                        logical_addr, expected_byte, actual_byte
                    );
                }
            }
        }
        
        if errors > max_errors_to_report {
            println!("  ... and {} more errors", errors - max_errors_to_report);
        }
        
        assert_eq!(errors, 0, "Found {} byte mismatches when reading back data", errors);
        
        println!("✓ Phase 7 Test 2: ROM images with random data passed");
        println!("  - Stored and read back all {} random bytes correctly", rom_size);
    }

    // Helper: Check if CS line is active at given address
    fn is_cs_active(gpio_value: u16, cs_pin: u8, active_low: bool) -> bool {
        let bit_value = (1 << cs_pin) & gpio_value;
        if active_low {
            bit_value == 0
        } else {
            bit_value != 0
        }
    }

    // ========================================================================
    // TEST 27: Multi ROM Set Images
    // ========================================================================
    
    #[test]
    fn test_phase7_multi_rom_set_images() {
        let json = r#"{
            "version": 1,
            "description": "Phase 7 multi ROM set test",
            "rom_sets": [{
                "type": "multi",
                "roms": [
                    {
                        "file": "rom0.bin",
                        "type": "2364",
                        "cs1": "active_low",
                        "cs2": "ignore",
                        "cs3": "ignore"
                    },
                    {
                        "file": "rom1.bin",
                        "type": "2364",
                        "cs1": "active_low",
                        "cs2": "ignore",
                        "cs3": "ignore"
                    },
                    {
                        "file": "rom2.bin",
                        "type": "2364",
                        "cs1": "active_low",
                        "cs2": "ignore",
                        "cs3": "ignore"
                    }
                ]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create distinct test data for each ROM
        let rom_size = 8192;
        let rom0_data = create_test_rom_data(rom_size, 0x11);
        let rom1_data = create_test_rom_data(rom_size, 0x22);
        let rom2_data = create_test_rom_data(rom_size, 0x33);
        
        builder.add_file(FileData { id: 0, data: rom0_data.clone() }).expect("Failed to add file 0");
        builder.add_file(FileData { id: 1, data: rom1_data.clone() }).expect("Failed to add file 1");
        builder.add_file(FileData { id: 2, data: rom2_data.clone() }).expect("Failed to add file 2");
        
        let props = default_fw_props();
        let board = props.board();
        let (_metadata_buf, rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Get CS pins
        let cs1_pin = board.pin_cs1(onerom_config::rom::RomType::Rom2364);
        let x1_pin = board.pin_x1();
        let x2_pin = board.pin_x2();
        println!("CS1 pin: {}, X1 pin: {}, X2 pin: {}", cs1_pin, x1_pin, x2_pin);
        
        assert_ne!(cs1_pin, 255, "CS1 pin must be defined");
        assert_ne!(x1_pin, 255, "X1 pin must be defined for multi ROM sets");
        assert_ne!(x2_pin, 255, "X2 pin must be defined for multi ROM sets");
        
        // All CS lines are active low in this test
        let cs1_active_low = true;
        let x1_active_low = true;
        let x2_active_low = true;
        
        let mut errors = 0;
        let max_errors_to_report = 10;
        
        // Check all 64KB addresses
        for address in 0..65536u32 {
            let address_u16 = address as u16;
            
            // Determine which CS lines are active
            let cs1_active = is_cs_active(address_u16, cs1_pin, cs1_active_low);
            let x1_active = is_cs_active(address_u16, x1_pin, x1_active_low);
            let x2_active = is_cs_active(address_u16, x2_pin, x2_active_low);
            
            let active_count = [cs1_active, x1_active, x2_active].iter().filter(|&&x| x).count();
            
            // Read actual byte from ROM images buffer
            let actual_byte = read_rom_byte_abs(&rom_images_buf, address as usize, board);
            
            let expected_byte = if active_count == 1 {
                // Exactly one CS active - should contain that ROM's data
                let rom_offset = (address as usize) & 0x1FFF; // Lower 13 bits for 8KB ROM
                if cs1_active {
                    rom0_data[rom_offset]
                } else if x1_active {
                    rom1_data[rom_offset]
                } else {
                    rom2_data[rom_offset]
                }
            } else {
                // Invalid (0 or multiple CS active) - should be 0xAA
                0xAA
            };
            
            if actual_byte != expected_byte {
                errors += 1;
                if errors <= max_errors_to_report {
                    println!(
                        "  Mismatch at addr=0x{:04X} (CS1={}, X1={}, X2={}): expected 0x{:02X}, got 0x{:02X}",
                        address, cs1_active, x1_active, x2_active, expected_byte, actual_byte
                    );
                }
            }
        }
        
        if errors > max_errors_to_report {
            println!("  ... and {} more errors", errors - max_errors_to_report);
        }
        
        assert_eq!(errors, 0, "Found {} byte mismatches in multi ROM set", errors);
        
        println!("✓ Phase 7 Test 3: Multi ROM set images passed");
        println!("  - Verified all 64KB with 3 ROMs selected by CS lines");
        println!("  - Validated invalid addresses contain 0xAA");
    }

    // ========================================================================
    // TEST 28: Banked ROM Set Images
    // ========================================================================
    
    #[test]
    fn test_phase7_banked_rom_set_images() {
        let json = r#"{
            "version": 1,
            "description": "Phase 7 banked ROM set test",
            "rom_sets": [{
                "type": "banked",
                "roms": [
                    {
                        "file": "bank0.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    },
                    {
                        "file": "bank1.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    },
                    {
                        "file": "bank2.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    },
                    {
                        "file": "bank3.rom",
                        "type": "2364",
                        "cs1": "active_low"
                    }
                ]
            }]
        }"#;
        
        let mut builder = Builder::from_json(json).expect("Failed to parse JSON");
        
        // Create distinct test data for each ROM (each 8KB)
        let rom_size = 8192;
        let rom_data = vec![
            create_test_rom_data(rom_size, 0x11),
            create_test_rom_data(rom_size, 0x22),
            create_test_rom_data(rom_size, 0x33),
            create_test_rom_data(rom_size, 0x44),
        ];
        
        for (id, data) in rom_data.iter().enumerate() {
            builder.add_file(FileData { 
                id, 
                data: data.clone() 
            }).expect(&format!("Failed to add file {}", id));
        }
        
        let props = default_fw_props();
        let board = props.board();
        let (_metadata_buf, rom_images_buf) = builder.build(props).expect("Build failed");
        
        // Get CS pins
        let cs1_pin = board.pin_cs1(onerom_config::rom::RomType::Rom2364);
        let x1_pin = board.pin_x1();
        let x2_pin = board.pin_x2();
        
        assert_ne!(cs1_pin, 255, "CS1 pin must be defined");
        assert_ne!(x1_pin, 255, "X1 pin must be defined for banked ROM sets");
        assert_ne!(x2_pin, 255, "X2 pin must be defined for banked ROM sets");
        
        let cs1_active_low = true;

        // We need to know which way X1/X2 are pulled when selected
        let x_dirn = board.x_jumper_pull();
        
        let mut errors = 0;
        let max_errors_to_report = 10;
        
        // For banked ROMs, the X1/X2 bits in the GPIO select which ROM
        for address in 0..65536u32 {
            let address_u16 = address as u16;
            
            let cs1_active = is_cs_active(address_u16, cs1_pin, cs1_active_low);
            let x1_bit = (address_u16 >> x1_pin) & 1;
            let x2_bit = (address_u16 >> x2_pin) & 1;
            
            // Determine which ROM based on X1/X2 bits and if CS1 is active
            let expected_byte = {
                let rom_offset = (address as usize) & 0x1FFF; // Lower 13 bits for 8KB ROM
                
                let mut bank = ((x2_bit << 1) | x1_bit) as usize;
                if x_dirn == 0 {
                    bank = 3 - bank;
                }
                
                if bank < rom_data.len() {
                    bank = bank % rom_data.len(); // Wrap around
                }
                rom_data[bank][rom_offset]
            };
            
            // Currently fill ROM section with banked ROM even if CS is INACTIVE.
            
            let actual_byte = read_rom_byte_abs(&rom_images_buf, address as usize, board);
            
            if actual_byte != expected_byte {
                errors += 1;
                if errors <= max_errors_to_report {
                    println!(
                        "  Mismatch at addr=0x{:04X} (CS1={}, X1={}, X2={}): expected 0x{:02X}, got 0x{:02X}",
                        address, cs1_active, x1_bit, x2_bit, expected_byte, actual_byte
                    );
                }
            }
        }
        
        if errors > max_errors_to_report {
            println!("  ... and {} more errors", errors - max_errors_to_report);
        }
        
        assert_eq!(errors, 0, "Found {} byte mismatches in banked ROM set", errors);
        
        println!("✓ Phase 7 Test 4: Banked ROM set images passed");
        println!("  - Verified all 64KB with {} ROMs in banks", rom_data.len());
        println!("  - Validated X1/X2 bit values select correct ROM");
    }
}