//! Linux Kernel headers related to the 32-bit boot protocol.

/// Offset of the `SetupHeader` in the Linux Kernel image.
pub const SETUP_HDR_OFFSET: u32 = 0x01f1;

/// Kernel attributes, used by the Linux Kernel during its setup.
#[repr(C, packed)]
pub struct SetupHeader {
    /// Size of the setup, in sectors.
    ///
    /// The real-mode code consists of the boot sector, plus the setup code.
    setup_sects: u8,

    /// If set, root is mounted read-only (deprecated, use `ro` or `rw` in command line instead)
    root_flags: u16,

    /// Size of the 32-bit code in 16-byte paragraphs.
    syssize: u32,

    /// Do not use
    ram_size: u16,

    /// Video mode control
    vid_mode: u16,

    /// Default root device number (deprecated, use `root=` in command line instead)
    root_dev: u16,

    /// 0xAA55 magic number
    boot_flag: u16,

    /// x86 JMP instruction
    jump: u16,

    /// Magic signature, should be "HdrS"
    header: u32,

    /// Boot protocol version supported (major << 8 + minor format)
    version: u16,

    /// Boot loader hook
    realmode_swtch: u32,

    /// Load-low segment (obsolete)
    start_sys_seg: u16,

    /// Pointer to Kernel version string (pointer to a nil-terminated string, less 0x200).
    ///
    /// If this value is set to 0x1c00, the string can be found at offset 0x1e00.
    kernel_version: u16,

    /// Bootloader identifier
    type_of_loader: u8,

    /// Boot protocol option flags
    ///
    /// Bit 0: `LOADED_HIGH`, if set the protected mode code is at 0x100000, or at 0x10000
    /// otherwise.
    ///
    /// Bit 1: `KASLR_FLAG`, if set KASLR is enabled.
    ///
    /// Bit 5: `QUIET_FLAG`, if set requests that the kernel doesn't print early messages.
    ///
    /// Bit 6: `KEEP_SEGMENTS`, if set does not reload the segments registers in the 32-bit entry
    /// point.
    ///
    /// Bit 7: `CAN_USE_HEAP`, set if the value entered in `heap_end_ptr` is valid.
    loadflags: u8,

    /// Move to high memory size
    setup_move_size: u16,

    /// Address to jump to in protected mode.
    code32_start: u32,

    /// initrd load address
    ramdisk_image: u32,

    /// initrd size
    ramdisk_size: u32,

    /// Do not use
    bootsect_kludge: u32,

    /// Free memory after setup end.
    ///
    /// Set this field to the offset, from the beginning of real-mode code, of the end of the setup
    /// stack/heap, minus 0x200.
    heap_end_ptr: u16,

    /// Extended bootloader version
    ext_loader_ver: u8,

    /// Extended bootloader ID
    ext_loader_type: u8,

    /// 32-bit pointer to the kernel command line
    cmd_line_ptr: u32,

    /// Highest legal initrd address.
    initrd_addr_max: u32,

    /// Physical address alignement required for the kernel
    kernel_alignement: u32,

    /// Whether kernel is relocatable or not.
    ///
    /// If non-zero, the kernel can be loaded at any address
    /// that satisfies the `kernel_alignement`. The bootloader must set the `code32_start` field to
    /// point to the loaded code (or to a bootloader hook).
    relocatable_kernel: u8,

    /// Minimum alignment, as a power of two
    min_alignment: u8,

    /// Boot protocol option flags
    ///
    /// Bit 0 (read): `XLF_KERNEL_64` if set, the kernel has the legacy 64-bit entry point at
    /// 0x200.
    ///
    /// Bit 1 (read): `XLF_CAN_BE_LOADED_ABOVE_4G` if set, the kernel/boot_params/cmdline/ramdisk
    /// can be loaded at memory addresses higher than 4G.
    ///
    /// Bit 2 (read): `XLF_EFI_HANDOVER_32` if set, the kernel supports the 32-bit EFI handoff
    /// entry point given at `handover_offset`.
    ///
    /// Bit 3 (read): `XLF_EFI_HANDOVER_64` if set, the kernel supports the 64-bit EFI handoff
    /// entry point given at `handover_offset + 0x200`.
    ///
    /// Bit 4 (read): `XLF_EFI_KEXEC` if set, the kernel supports kexec EFI boot.
    xloadflags: u16,

    /// Maximum size of the kernel command line
    cmdline_size: u32,

    /// Hardware subarchitecture (if we are in a paravirtualized environment)
    hardware_subarch: u32,

    /// Subarchitecture-specific data
    hardware_subarch_data: u64,

    /// Offset of kernel payload.
    ///
    /// If non-zero, contains the offset from the beginning of the protected-mode code to the
    /// payload, that might be compressed (the format of the data should be determined using the
    /// standard magic numbers).
    payload_offset: u32,

    /// Length of kernel payload
    payload_length: u32,

    /// 64-bit physical pointer to linked list of struct `setup_data`
    setup_data: u64,

    /// Preferred loading address
    pref_address: u64,

    /// Linear memory required during initialization
    init_size: u32,

    /// Offset of handover entry point
    handover_offset: u32,

    /// Offset of the `kernel_info`
    kernel_info_offset: u32,
}

impl SetupHeader {
    /// Copies the `SetupHeader` from a Kernel image.
    ///
    /// Loads the header that is expected to be at offset `SETUP_HDR_OFFSET` from the start ot the
    /// image, and copies it. It means that modifying the returned `SetupHeader` struct does not modify
    /// the real kernel's setup header in place, and it has to be copied back when all fields have been
    /// properly set by the bootloader.
    ///
    /// # Safety
    ///
    /// Excepts a valid Kernel header at the offset `0x1f1` from the provided `starting_addr`, as
    /// described by the `SetupHeader` fields.
    pub fn copy_from_image(starting_addr: *mut u8) -> Self {
        let header_addr = starting_addr.wrapping_offset(0x01f1);
        unsafe { core::ptr::read(header_addr as *mut Self) }
    }
}

#[repr(C, packed)]
pub struct ScreenInfo {
    orig_x: u8,
    orig_y: u8,
    ext_mem_k: u16,
    orig_video_page: u16,
    orig_video_mode: u8,
    orig_video_cols: u8,
    flags: u8,
    unused: u8,
    orig_video_lines: u8,
    orig_video_isvga: u8,
    orig_video_points: u16,
    lfb_width: u16,
    lfb_height: u16,
    lfb_depth: u16,
    lfb_base: u32,
    lfb_size: u32,
    cl_magic: u16,
    lfb_linelength: u16,
    red_size: u8,
    red_pos: u8,
    green_size: u8,
    green_pos: u8,
    blue_size: u8,
    blue_pos: u8,
    rsvd_size: u8,
    rsvd_pos: u8,
    vesapm_seg: u16,
    vesapm_off: u16,
    pages: u16,
    vesa_attributes: u16,
    capabilities: u32,
    ext_lfb_base: u32,
    reserved: u16,
}

#[repr(C, packed)]
pub struct ApmBiosInfo {
    version: u16,
    cseg: u16,
    offset: u32,
    cseg_16: u16,
    dseg: u16,
    flags: u16,
    cseg_len: u16,
    cseg_16_len: u16,
    dseg_len: u16,
}

#[repr(C, packed)]
pub struct IstInfo {
    signature: u32,
    command: u32,
    event: u32,
    perf_level: u32,
}

#[repr(C)]
pub struct SysDescTable {
    length: u16,
    table: [u8; 14],
}

#[repr(C, packed)]
pub struct OlpcOfwHeader {
    /// OFW signature
    ofw_magic: u32,
    ofw_version: u32,

    /// Callback into OFW
    cif_handler: u32,
    irq_desc_table: u32,
}

#[repr(C)]
pub struct EfiInfo {
    efi_loader_signature: u32,
    efi_systab: u32,
    efi_memdesc_size: u32,
    efi_memdesc_version: u32,
    efi_memmap: u32,
    efi_memmap_size: u32,
    efi_systab_hi: u32,
    efi_memmap_hi: u32,
}

#[repr(C, packed)]
pub struct BootE820Entry {
    addr: u64,
    size: u64,
    entry_type: u32,
}

#[repr(C, packed)]
pub struct EddInfo {
    device: u8,
    version: u8,
    interface_support: u16,
    legacy_max_cylinder: u16,
    legacy_max_head: u8,
    legacy_sectors_per_track: u8,
    params: EddDeviceParams,
}

#[repr(C, packed)]
pub struct EddDeviceParams {
    length: u16,
    info_flags: u16,
    num_default_cylinders: u32,
    num_default_heads: u32,
    sectors_per_track: u32,
    number_of_sectors: u64,
    bytes_per_sector: u16,
    dpte_ptr: u32,
    key: u16,
    device_path_info_length: u8,
    reserved: u8,
    reserved2: u16,
    host_bus_type: [u8; 4],
    interface_type: [u8; 8],
    interface_path: u64,
    device_path: u64,
    reserved3: u8,
    checksum: u8,
}

/// Additional field in the `boot_params` struct.
///
/// Used in the 32-bit Linux/x86 boot protocol.
#[repr(C, packed)]
pub struct ZeroPage {
    /// Text mode or framebuffer information
    screen_info: ScreenInfo,

    /// APM BIOS information
    apm_bios_info: ApmBiosInfo,
    padding: u32,

    /// Physical address of tboot shared page
    tboot_addr: u64,

    /// Intel SpeedStep (IST) BIOS support information
    ist_info: IstInfo,

    acpi_rsdp_addr: u64,

    padding2: [u8; 8],

    /// hd0 disk parameter (outdated)
    hd0_info: [u8; 16],

    /// hd1 disk parameter (outdated)
    hd1_info: [u8; 16],

    /// System description table (outdated)
    sys_desc_table: SysDescTable,

    /// OLPC OpenFirmware CIF
    olpc_ofw_header: OlpcOfwHeader,

    /// Ramdisk image high 32 bits
    ext_ramdisk_image: u32,

    /// Ramdisk size high 32 bits
    ext_ramdisk_size: u32,

    /// `cmd_line_ptr` high 32 bits
    ext_cmd_line_ptr: u32,

    padding3: [u8; 112],

    cc_blob_address: u32,

    /// Video mode setup
    edid_info: [u8; 128],

    /// EFI 32 information
    efi_info: EfiInfo,

    /// Alternative mem check (in Kb)
    alt_mem_k: u32,

    /// Scratch field for the kernel setup code
    scratch: u32,

    /// Number of entries in E820 Table
    e820_entries: u8,

    /// Number of entries in eddbuf
    eddbuf_entries: u8,

    /// Number of entries in edd_mbr_sig_buffer
    edd_mbr_sig_buffer_entries: u8,

    /// Numlock is enabled
    kdb_status: u8,

    /// Secure boot is enabled in the firmware
    secure_boot: u8,

    padding4: [u8; 2],

    /// Sentinel (to detect broken bootloaders, hopefully not us)
    sentinel: u8,

    padding5: u8,

    hdr: SetupHeader,

    padding6: [u8; 159 - core::mem::size_of::<SetupHeader>()],

    /// EDD MBR signatures
    edd_mbr_sig_buffer: [u32; 16],

    /// E820 memory map table
    e820_table: [BootE820Entry; 128],

    padding7: [u8; 48],

    eddbuf: [EddInfo; 6],

    padding8: [u8; 276],
}
