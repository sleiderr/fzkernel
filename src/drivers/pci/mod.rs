use core::mem;

use alloc::vec::Vec;
use conquer_once::spin::OnceCell;

use crate::{
    drivers::pci::device::{PCIDevice, PCIDevices},
    info,
    io::{inl, outl},
    println,
};

pub mod device;

/// List of available PCI devices, after initial enumeration
pub static PCI_DEVICES: OnceCell<PCIDevices> = OnceCell::uninit();

/// Builds the [`DeviceClass`] enum containing known PCI device classes.
macro_rules! pci_device_class_def {
    ($( ($class: literal, $name: ident, $desc: tt)), *) => {
        #[derive(Debug, Clone, Copy)]
        pub enum DeviceClass {
            $(
            #[doc = $desc]
            $name,
            )*
            Unknown(u32)
        }

        impl core::fmt::Display for DeviceClass {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let desc = match &self {
                    $(DeviceClass::$name => $desc,)*
                    DeviceClass::Unknown(_) => "Unknown",
                };

                core::write!(f, "{}", desc)
            }
        }

        impl From<DeviceClass> for u32 {
            fn from(value: DeviceClass) -> u32 {
                match value {
                    $(DeviceClass::$name => $class,)*
                    DeviceClass::Unknown(val) => val,
                }
            }
        }

        impl From<u32> for DeviceClass {
            fn from(value: u32) -> DeviceClass {
                match value {
                    $($class => Self::$name,)*
                    val => Self::Unknown(val)
                }
            }
        }
    };
}

pci_device_class_def!(
    (0x000100, VGACompatible, "VGA-compatible device"),
    (
        0x010000,
        SCSIControllerVendor,
        "SCSI controller - vendor-specific interface"
    ),
    (0x010011, SCSIStorageDevice, "SCSI storage device (PQI)"),
    (0x010012, SCSIController, "SCSI controller"),
    (
        0x010013,
        SCSIStorageDeviceAndController,
        "SCSI storage device and SCSI controller"
    ),
    (
        0x010021,
        SCSIStorageDeviceNVME,
        "SCSI storage device (NVME)"
    ),
    (0x010100, IDEController, "IDE controller"),
    (0x010200, FloppyController, "Floppy disk controller"),
    (0x010300, IPIBusController, "IPI bus controller"),
    (0x010400, RAIDController, "RAID controller"),
    (
        0x010520,
        ATAControllerADMASSTEP,
        "ATA controller with ADMA interface - single stepping"
    ),
    (
        0x010530,
        ATAControllerADMACTOP,
        "ATA controller with ADMA interface - continuous operation"
    ),
    (
        0x010600,
        SATAControllerVendor,
        "Serial ATA controller - vendor-specific interface"
    ),
    (
        0x010601,
        SATAControllerAHCI,
        "Serial ATA controller - AHCI interface"
    ),
    (0x010602, SerialStorageBuf, "Serial Storage Bus Interface"),
    (
        0x010700,
        SASController,
        "Serial Attached SCSI (SAS) controller"
    ),
    (
        0x010800,
        NVMSubsystem,
        "NVM subsystem - vendor-specific interface"
    ),
    (
        0x010801,
        NVMSubsystemNVMHCI,
        "NVM subsystem - NVMHCI interface"
    ),
    (0x010802, NVMEIOController, "NVMe I/O controller"),
    (
        0x010803,
        NVMEAdminController,
        "NVMe administrative controller"
    ),
    (0x010900, UFSController, "UFS controller"),
    (
        0x010901,
        UFSHCI,
        "Universal Flash Storage Host Controller Interface"
    ),
    (
        0x018000,
        UnknownMassStorageController,
        "Mass storage controller (unknown)"
    ),
    (0x020000, EthernetController, "Ethernet controller"),
    (0x020100, TokenRingController, "Token Ring controller"),
    (0x020200, FDDIController, "FDDI controller"),
    (0x020300, ATMController, "ATM controller"),
    (0x020400, ISDNController, "ISDN controller"),
    (0x020500, WorldFipController, "WorldFip controller"),
    (0x020600, PICMGMultiComputing, "PICMG 2.14 Multi Computing"),
    (0x020700, InfiniBandController, "InfiniBand controller"),
    (0x020800, HostFabricController, "Host fabric controller"),
    (
        0x028000,
        UnknownNetworkController,
        "Network controller (unknown)"
    ),
    (
        0x030000,
        VGACompatibleController,
        "VGA-compatible controller"
    ),
    (
        0x030001,
        Compatible8514Controller,
        "8514-compatible controller"
    ),
    (0x030100, XGAController, "XGA controller"),
    (0x030200, Controller3D, "3D controller"),
    (
        0x038000,
        UnknownDisplayController,
        "Display controller (unknown)"
    ),
    (0x040000, VideoDevice, "Video device"),
    (0x040100, AudioDevice, "Audio device"),
    (
        0x040200,
        ComputerTelephonyDevice,
        "Computer telephony device"
    ),
    (
        0x040300,
        HDACompatible,
        "High Definition Audio 1.0 compatible"
    ),
    (
        0x040380,
        HDACompatibleAdd,
        "High Definition Audio 1.0 compatible with additional extensions"
    ),
    (
        0x048000,
        UnknownMultimediaDevice,
        "Multimedia device (unknown)"
    ),
    (0x050000, RAM, "RAM"),
    (0x500100, Flash, "Flash"),
    (
        0x508000,
        UnknownMemoryController,
        "Memory controller (unknown)"
    ),
    (0x060000, HostBridge, "Host bridge"),
    (0x060100, ISABridge, "ISA bridge"),
    (0x060200, EISABridge, "EISA bridge"),
    (0x060300, MCABridge, "MCA bridge"),
    (0x060400, PciPciBridge, "PCI-to-PCI bridge"),
    (
        0x060401,
        SubDecodePciPciBridge,
        "Subtractive Decode PCI-to-PCI bridge"
    ),
    (0x060500, PCMCIABridge, "PCMCIA bridge"),
    (0x060600, NubusBridge, "NuBus bridge"),
    (0x060700, CardBusBridge, "CardBus bridge"),
    (0x060800, RacewayBridge, "RACEway bridge"),
    (
        0x060940,
        SemiTransparentPciPciBridgePrim,
        "Semi-transparent PCI-to-PCI bridge with the primary
     PCI bus side facing the system host processor"
    ),
    (
        0x060980,
        SemiTransparentPciPciBridgeSec,
        "Semi-transparent PCI-to-PCI bridge
     with the secondary PCI bus side facing the system host processor"
    ),
    (
        0x060A00,
        InfiniBandPciHostBridge,
        "InfiniBand-to-PCI host bridge"
    ),
    (
        0x060B00,
        AdvancedSwitchingPciBridgeCustom,
        "Advanced Switching to PCI host bridge–Custom Interface"
    ),
    (
        0x060B01,
        AdvancedSwitchingPciBridgeASISIG,
        "Advanced Switching to PCI host bridge– ASI-SIG Defined Portal Interface"
    ),
    (0x068000, UnknownBridgeDevice, "Bridge device (unknown)"),
    (
        0x070000,
        GenericXTSerialController,
        "Generic XT-compatible serial controller"
    ),
    (
        0x070001,
        Compatible16450SerialController,
        "16450-compatible serial controller"
    ),
    (
        0x070002,
        Compatible16550SerialController,
        "16550-compatible serial controller"
    ),
    (
        0x070003,
        Compatible16650SerialController,
        "16650-compatible serial controller"
    ),
    (
        0x070004,
        Compatible16750SerialController,
        "16750-compatible serial controller"
    ),
    (
        0x070005,
        Compatible16850SerialController,
        "16850-compatible serial controller"
    ),
    (
        0x070006,
        Compatible16950SerialController,
        "16950-compatible serial controller"
    ),
    (0x070100, ParallelPort, "Parallel port"),
    (0x070101, BiParallelPort, "Bi-directional parallel port"),
    (
        0x070102,
        ECPCompliantParallelPort,
        "ECP 1.X compliant parallel port"
    ),
    (0x070103, IEEE1284Controller, "IEEE1284 controller"),
    (0x0701fe, IEEE1284TargetDevice, "IEEE1284 target device"),
    (
        0x070200,
        MultiportSerialController,
        "Multiport serial controller"
    ),
    (0x070300, GenericModem, "Generic modem"),
    (
        0x070301,
        Hayes16450Compatible,
        "Hayes compatible modem, 16450-compatible interface"
    ),
    (
        0x070302,
        Hayes16550Compatible,
        "Hayes compatible modem, 16550-compatible interface"
    ),
    (
        0x070303,
        Hayes16650Compatible,
        "Hayes compatible modem, 16650-compatible interface"
    ),
    (
        0x070304,
        Hayes16750Compatible,
        "Hayes compatible modem, 16750-compatible interface"
    ),
    (0x070400, GPIBController, "GPIB (IEEE 488.1/2) controller"),
    (0x070500, SmartCard, "Smart Card"),
    (
        0x078000,
        UnknownCommunicationDevice,
        "Communication device (unkown)"
    ),
    (0x080000, Generic8259PIC, "Generic 8259 PIC"),
    (0x800001, ISAPIC, "ISA PIC"),
    (0x800002, EISAPIC, "EISA PIC"),
    (
        0x800010,
        IOAPICInterruptController,
        "I/O APIC interrupt controller"
    ),
    (
        0x080020,
        IOxAPICInterruptController,
        "I/O(x) APIC interrupt controller"
    ),
    (
        0x080100,
        Generic8237DMAController,
        "Generic 8237 DMA controller"
    ),
    (0x080101, ISADMAController, "ISA DMA controller"),
    (0x080102, EISADMAController, "EISA DMA controller"),
    (
        0x080200,
        Generic8254SystemTimer,
        "Generic 8254 system timer"
    ),
    (0x080201, ISASystemTimer, "ISA system timer"),
    (0x080202, EISASystemTimer, "EISA system timers (two timers)"),
    (0x080203, HPETimer, "High Performance Event Timer"),
    (0x080300, GenericRTCController, "Generic RTC controller"),
    (0x080301, ISARTCController, "ISA RTC controller"),
    (
        0x080400,
        GenericPCIHPController,
        "Generic PCI Hot-Plug controller"
    ),
    (0x080500, SDHostController, "SD Host controller"),
    (0x080600, IOMMU, "IOMMU"),
    (
        0x080700,
        RootComplexEventCollector,
        "Root Complex Event Collector"
    ),
    (
        0x088000,
        UnknownSystemPeripheral,
        "System peripheral (unknown)"
    ),
    (0x090000, KeyboardController, "Keyboard controller"),
    (0x090100, Digitizer, "Digitizer (pen)"),
    (0x090200, MouseController, "Mouse controller"),
    (0x090300, ScannerController, "Scanner controller"),
    (
        0x090400,
        GameportControllerGeneric,
        "Gameport controller (generic)"
    ),
    (0x090401, GameportController, "Gameport controller"),
    (
        0x098000,
        UnknownInputController,
        "Input controller (unknown)"
    ),
    (0x0A0000, GenericDockingStation, "Generic docking station"),
    (0x0A8000, UnknownDockingStation, "Docking station (unknown)"),
    (0x0B0000, CPU386, "CPU 386"),
    (0x0B0100, CPU486, "CPU 486"),
    (0x0B0200, Pentium, "Pentium"),
    (0x0B1000, Alpha, "Alpha"),
    (0x0B2000, PowerPC, "PowerPC"),
    (0x0B3000, MIPS, "MIPS"),
    (0x0B4000, Coprocessor, "Co-processor"),
    (0x0B8000, UnknownCPU, "CPU (unknown)"),
    (0x0C0000, IEEE1394Firewire, "IEEE 1394 (FireWire)"),
    (
        0x0C0010,
        IEEE1394OpenHCI,
        "IEEE 1394 following the 1394 OpenHCI specification"
    ),
    (0x0C0100, ACCESSBus, "ACCESS.bus"),
    (0x0C0200, SSA, "SSA"),
    (
        0x0C0300,
        USBUHCSpec,
        "USB (following the Universal Host Controller Specification)"
    ),
    (
        0x0C0310,
        USBOHCSpec,
        "USB (following the Open Host Controller Specification)"
    ),
    (
        0x0C0320,
        USB2IEHCISpec,
        "USB 2 host controller (following the Intel Enhanced Host Controller Interface Specification)"
    ),
    (
        0x0C0330,
        USBxHCI,
        "USB (following the Intel eXtensible Host Controller Interface Specification)"
    ),
    (
        0x0C0380,
        USBUnspecified,
        "USB with no specific Programming Interface"
    ),
    (0x0C03FE, USBDevice, "USB device"),
    (0x0C0400, FibreChannel, "Fibre Channel"),
    (0x0C0500, SMBus, "SMBus (System Management Bus)"),
    (0x0C0600, InfiniBand, "InfiniBand (deprecated)"),
    (0x0C0700, IPMISMICInterface, "IPMI SMIC Interface"),
    (
        0x0C0701,
        IPMIKeyboardControllerStyleIf,
        "IPMI Keyboard Controller Style Interface"
    ),
    (
        0x0C0702,
        IPMIBlockTransferInterface,
        "IPMI Block Transfer Interface"
    ),
    (
        0x0C0800,
        SERCOSInterfaceStandard,
        "SERCOS Interface Standard"
    ),
    (0x0C09AA, CANbus, "CANbus"),
    (
        0x0C0A00,
        MIPII3CHostController,
        "MIPI I3CSM Host Controller Interface"
    ),
    (
        0x0C8000,
        UnknownSerialBusController,
        "Serial bus controller (unknown)"
    ),
    (
        0x0D0000,
        IRDACompatibleController,
        "iRDA compatible controller"
    ),
    (0x0D0100, ConsumerIRController, "Consumer IR controller"),
    (0x0D0110, UWBRadioController, "UWB Radio controller"),
    (0x0D1000, RFController, "RF controller"),
    (0x0D1100, BluetoothController, "Bluetooth"),
    (0x0D1200, Broadband, "Broadband"),
    (0x0D2000, Ethernet5, "Ethernet (802.11a – 5 GHz)"),
    (0x0D2100, Ethernet2, "Ethernet (802.11b – 2.4 GHz)"),
    (0x0D4000, CellularController, "Cellular controller/modem"),
    (
        0x0D4100,
        CellularControllerAndEthernet,
        "Cellular controller/modem plus Ethernet (802.11)"
    ),
    (
        0x0D8000,
        UnknownWirelessController,
        "Wireless controller (unknown)"
    ),
    (0x0E0000, MessageFIFO040, "Message FIFO at offset 040"),
    (0x0F0100, TV, "TV"),
    (0x0F0200, SatelliteAudio, "Audio"),
    (0x0F0300, SatelliteVoice, "Voice"),
    (0x0F0400, SatelliteData, "Data"),
    (
        0x0F8000,
        UnknownSatelliteController,
        "Satellite communication controller (unknown)"
    ),
    (
        0x100000,
        NetworkComputingEncryptionController,
        "Network and computing encryption and decryption controller"
    ),
    (
        0x101000,
        EntertainmentEncryptionController,
        "Entertainment encryption and decryption controller"
    ),
    (
        0x108000,
        UnknownEncryptionController,
        "Other encryption and decryption controller"
    ),
    (0x110000, DPIOModule, "DPIO modules"),
    (0x110100, PerformanceCounter, "Performance counters"),
    (
        0x111000,
        CommunicationSync,
        "Communications synchronization plus time and frequency test/measurement"
    ),
    (0x112000, ManagementCard, "Management card"),
    (
        0x118000,
        SignalProcessingController,
        "Signal processing / data acquisition controller (unknown)"
    ),
    (0x120000, ProcessingAccelerator, "Processing Accelerator"),
    (
        0x130000,
        NonEssentialInstrumentation,
        "Non-Essential Instrumentation Function"
    )
);

/// Configuration Space Header for a PCI device.
///
/// The first 16 bytes of the header are common to every layout, and the last 48 bytes can vary
/// depending on the type of the entry.
#[repr(C)]
#[derive(Debug)]
pub struct PCIHeader {
    common: PCICommonHeader,
    var: PCIHeaderVar,
}

/// Various layout for the second part of the PCI Header
#[derive(Debug)]
pub enum PCIHeaderVar {
    /// Oh header type
    Type0(PCIHeaderType0),

    /// 01h header type (PCI-PCI bridge)
    Type1(PCIHeaderType1),

    /// 02h header type (CardBus bridge)
    Type2(PCIHeaderType2),
}

/// Basic PCI specific header layout (00h)
#[repr(C)]
#[derive(Debug)]
pub struct PCIHeaderType0 {
    /// Base address #0
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00b, the register is 32-bits wide, if they are 10b, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_0: u32,

    /// Base address #1
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_1: u32,

    /// Base address #2
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_2: u32,

    /// Base address #3
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_3: u32,

    /// Base address #4
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_4: u32,

    /// Base address #5
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_5: u32,

    /// CardBus CIS Pointer.
    ///
    /// Pointer to Card Information Structure (used by devices that share silicon between PCI and
    /// CardBus).
    cardbus_cis_ptr: u32,

    /// Subsystem vendor ID
    ///
    /// Used to uniquely identify the expansion board or subsystem where the PCI device resides.
    subsystem_vendor_id: u16,

    /// Subsystem ID
    ///
    /// Used to uniquely identify the expansion board or subsystem where the PCI device resides.
    subsystem_id: u16,

    /// Expansion ROM base address
    ///
    /// Some PCI devices require local EPROMs for expansion ROM. Information about this expansion
    /// ROM are encoded in this register.
    ///
    /// This behaves like a BAR, but the encoding of the bottom bits is different. The upper 21
    /// bits correspond to the upper 21 bits of the expansion ROM base address.
    ///
    /// The address space required can be obtained by writing 1s to the address portion of the
    /// register and then reading the value back.
    ///
    /// `Bit 0`: Controls whether or not the device accepts accesses to its expansion ROM (if set)
    rom_base_addr: u32,

    /// Capabilities Pointer.
    ///
    /// Points to a linked list of new capabilities implemented by the device.
    /// Used if bit 4 of the `status` register is set. The bottom two bits are reserved (
    /// and should therefore be masked).
    cap_ptr: u8,
    reserved1: u8,
    reserved2: u16,
    reserved3: u32,

    /// Specifies which input of the system interrupt controllers the device's interrupt pin is
    /// connected to.
    ///
    /// For x86, this corresponds to PIC IRQ 0-15 (and not I/O APIC IRQ numbers).
    interrupt_line: u8,

    /// Specifies which interrupt pin the device uses.
    ///
    /// `0x1` corresponds to `INTA#`, `0x2` to `INTB#` and so on.
    /// 0 means the device does not use an interrupt pin.
    interrupt_pin: u8,

    /// Specifies the burst period length, in quarter of microseconds, that the devices need.
    min_grant: u8,

    /// Specifies how often the device needs to access the PCI bus, in quarter of microseconds.
    max_latency: u8,
}

/// PCI-PCI bridge header layout (type 01h)
#[repr(C)]
#[derive(Debug)]
pub struct PCIHeaderType1 {
    /// Base address #0
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_0: u32,

    /// Base address #1
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_1: u32,

    /// Stores the bus number of the PCI bus segment to which the primary interface of the bridge
    /// is connected. Software programs the value in this register.
    primary_bus: u8,

    /// Stores the bus number of the PCI bus segment to which the secondary interface of the bridge
    /// is connected. Software programs the value in this register.
    secondary_bus: u8,

    /// Stores the bus number of the highest numbered PCI bus segment which is behind (or
    /// subordinate to) the bridge. Software programs the value in this register..=
    subordinate_bus: u8,

    /// Specifies, in units of PCI bus clocks, the value of the Latency Timer for this PCI bus
    /// master. Applies to the secondary interface of a bridge.
    secondary_latency: u8,

    /// (Optional) Specifies the base address of the address range used by the bridge to determine
    /// when to forward I/O transactions from one interface to the other.
    ///
    /// If a bridge does not implement an I/O address range, the field must be implemented as a
    /// read-only register that return zero when read.
    io_base: u8,

    /// (Optional) Specifies the base address of the address range used by the bridge to determine
    /// when to forward I/O transactions from one interface to the other.
    ///
    /// If a bridge does not implement an I/O address range, the field must be implemented as a
    /// read-only register that return zero when read.
    io_limit: u8,

    /// Similar to the `Status` register, but reflects status conditions of the secondary interface
    /// (instead of the primary one).
    secondary_status: u16,

    /// Defines a memory-mapped address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 0s.
    ///
    /// The bottom 4 bits are read-only and return 0 when read.
    memory_base: u16,

    /// Defines a memory-mapped address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 1s.
    ///
    /// The bottom 4 bits are read-only and return 0 when read.
    memory_limit: u16,

    /// `(Optional)` Defines a prefetchable memory address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// If a bridge does not implement a prefetchable memory address range,
    /// `prefetchable_memory_base` must be implemented as a read-only register that returns 0 when
    /// read.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 0s.
    ///
    /// The bottom 4 bits encode whether or not the bridge supports 64-bit addresses.
    prefetchable_memory_base: u16,

    /// `(Optional)` Defines a prefetchable memory address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// If a bridge does not implement a prefetchable memory address range,
    /// `prefetchable_memory_base` must be implemented as a read-only register that returns 0 when
    /// read.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 1.
    ///
    /// The bottom 4 bits encode whether or not the bridge supports 64-bit addresses.
    prefetchable_memory_limit: u16,

    /// If `prefetchable_memory_base` indicates support for 32-bit addressing, this is implemented
    /// as a read-only register that returns 0 when read.
    ///
    /// If it indicates support for 64-bit addressing, this specifies the upper 32-bits `[63::32]`
    /// of the 64-bit base address that specifies the prefetchable memory address range.
    prefetchable_base_hi: u32,

    /// If `prefetchable_memory_base` indicates support for 32-bit addressing, this is implemented
    /// as a read-only register that returns 0 when read.
    ///
    /// If it indicates support for 64-bit addressing, this specifies the upper 32-bits `[63::32]`.
    /// of the 64-bit limit address that specifies the prefetchable memory address range.
    prefetchable_limit_hi: u32,

    /// If the `io_base` register indicates support for 16-bit addressing, this is implemented as a
    /// read-only register that returns 0 when read.
    ///
    /// If it indicates support for 32-bit addressing, this specifies the upper 16-bits `[31::16]`,
    /// of the 32-bit base address that specifies the I/O address range.
    io_base_hi: u16,

    /// If the `io_base` register indicates support for 16-bit addressing, this is implemented as a
    /// read-only register that returns 0 when read.
    ///
    /// If it indicates support for 32-bit addressing, this specifies the upper 16-bits `[31::16]`,
    /// of the 32-bit limit address that specifies the I/O address range.
    io_limit_hi: u16,

    /// `(Optional)` Points to a linked list of additional capabilities implemented by the device.
    ///
    /// If the `Capabilities List` bit (bit 4) in the status register is zero, the default state of
    /// this register is zero after reset. Otherwise, this register should be implemented as a
    /// read-only register.
    cap: u8,
    reserved1: u8,
    reserved2: u16,

    /// `(Optional)` Expansion rom base address
    expansion_rom_base_addr: u32,

    /// Read/Write register used to communicate interrupt line routing information between
    /// initialization code and the device driver.
    ///
    /// The value written specifies the routing of the device's `INTx#` pin to the system interrupt
    /// controllers.
    interrupt_line: u8,

    /// R/O register, used to indicate which interrupt pin the bridge uses. A value of 1 correspond
    /// to `INTA#` and so on.
    ///
    /// A bridge that does not implement any interrupt pins must return 0.
    interrupt_pin: u8,

    /// Provides extensions to the `command` register specific to a bridge.
    bridge_control: u16,
}

/// CardBus bridge header (type 02h)
#[repr(C)]
#[derive(Debug)]
pub struct PCIHeaderType2 {
    cardbus_sock_base_addr: u32,
    offset_cap_list: u8,
    reserved1: u8,
    secondary_status: u16,
    pci_bus_number: u8,
    cardbus_bus_number: u8,
    subordinate_bus_number: u8,
    cardbus_latency_timer: u8,
    mem_base_addr_0: u32,
    mem_limit_0: u32,
    mem_base_addr_1: u32,
    mem_limit_1: u32,
    io_base_addr_0: u32,
    io_limit_0: u32,
    io_base_addr_1: u32,
    io_limit_1: u32,
    interrupt_line: u8,
    interrupt_pin: u8,
    bridge_ctrl: u16,
    subsystem_device_id: u16,
    subsystem_vendor_id: u16,
    legacy_mode_base_addr: u32,
}

/// Common part of the Configuration Space Header
#[repr(C)]
#[derive(Debug)]
pub struct PCICommonHeader {
    /// Identifies the manufacturer of the device.
    vendor_id: u16,

    /// Identifies the particular device, and is assigned by the vendor.
    device_id: u16,

    /// Controls the behaviour on the interface.
    command: u16,

    /// Provides information about the interface.
    status: u16,

    /// Provides a device-specific revision identifier.
    revision_id: u8,

    /// Progamming interface, used to identify the function of the device
    prog_if: u8,

    /// Sub-class code, used to identify the function of the device
    subclass: u8,

    /// Base class code, used to identify the function of the device
    class_code: u8,

    /// Used when terminating a transaction that uses the `Memory Write and Invalidate` command and
    /// when prefetching.
    ///
    /// Only cacheline sizes that are power of two are valid.
    cache_line_size: u8,

    /// Required if a bridge is capable of burst transfer of more than two data phases on its
    /// interface. If implemented, that register is a R/W register,
    latency_timer: u8,

    /// Indicates the layout of the second part of the header.
    header_type: u8,

    /// `(Optional)` Used for control and status reporting of built-in self test capabilities.
    ///
    /// If `BIST` is not implemented, this register is read-only and returns 0 when read.
    bist: u8,
}

impl PCIHeader {
    /// Checks if the corresponding PCI device is present or not.
    pub fn is_present(&self) -> bool {
        self.common.vendor_id != 0xffff
    }

    /// Checks if the corresponding PCI device is multifunction or not.
    pub fn is_multifunction(&self) -> bool {
        self.common.header_type & 0x80 != 0
    }

    /// Loads a PCI device's header from its bus and device number.
    ///
    /// Always returns a header, even if the device is non-present, so a presence check should be
    /// performed after loading the `PCIHeader`.
    pub fn read(bus: u8, device: u8, function: u8) -> Self {
        let mut conf_header = [0u32; 4];
        (0..4).for_each(|i| {
            conf_header[i] = pci_read_long(bus, device, function, i as u8);
        });

        let common = unsafe { mem::transmute::<[u32; 4], PCICommonHeader>(conf_header) };

        let var = match common.header_type & 0x7f {
            2 => {
                let mut var_header = [0u32; 14];
                (0..14).for_each(|i| {
                    var_header[i] = pci_read_long(bus, device, function, i as u8 + 4);
                });
                PCIHeaderVar::Type2(unsafe {
                    mem::transmute::<[u32; 14], PCIHeaderType2>(var_header)
                })
            }
            1 => {
                let mut var_header = [0u32; 12];
                (0..12).for_each(|i| {
                    var_header[i] = pci_read_long(bus, device, function, i as u8 + 4);
                });
                PCIHeaderVar::Type1(unsafe {
                    mem::transmute::<[u32; 12], PCIHeaderType1>(var_header)
                })
            }
            _ => {
                let mut var_header = [0u32; 12];
                (0..12).for_each(|i| {
                    var_header[i] = pci_read_long(bus, device, function, i as u8 + 4);
                });
                PCIHeaderVar::Type0(unsafe {
                    mem::transmute::<[u32; 12], PCIHeaderType0>(var_header)
                })
            }
        };

        Self { common, var }
    }
}

pub fn pci_enumerate() {
    info!("pci", "beginning PCI enumeration");
    PCI_DEVICES.init_once(pci_enumerate_traversal);

    let devices = unsafe { PCI_DEVICES.get_unchecked() };

    for device in devices.iter() {
        info!("pci", "found {:}", device);
    }
}

/// Performs a recursive PCI devices discovery.
///
/// Assumes that PCI bridges between buses were properly set up beforehand.
pub fn pci_enumerate_traversal() -> PCIDevices {
    let mut list = Vec::new();
    let pci_host_0 = PCIHeader::read(0, 0, 0);

    if !pci_host_0.is_multifunction() {
        // Only one PCI host controller
        pci_bus_scan(0, &mut list);
    } else {
        // Multiple PCI host controller
        for func in 1..8 {
            let pci_aux_host = PCIHeader::read(0, 0, func);
            if !pci_aux_host.is_present() {
                break;
            }
            pci_bus_scan(func, &mut list);
        }
    }

    PCIDevices::from_devices(list)
}

/// Checks if the function is a PCI to PCI bridge, and checks the secondary bus of the bridge.
pub(super) fn pci_function_secbus_check(
    bus: u8,
    device: u8,
    function: u8,
    list: &mut Vec<PCIDevice>,
) {
    let header = PCIHeader::read(bus, device, function);
    if !header.is_present() {
        return;
    }

    if (header.common.class_code == 0x6) && (header.common.subclass == 0x4) {
        if let PCIHeaderVar::Type1(bridge) = header.var {
            let secondary_bus = bridge.secondary_bus;
            pci_bus_scan(secondary_bus, list);
        }
    }

    list.push(PCIDevice::load(bus, device, function));
}

/// Scans every slot of one `bus` for connected devices.
pub(super) fn pci_bus_scan(bus: u8, list: &mut Vec<PCIDevice>) {
    for device in 0..32 {
        let header = PCIHeader::read(bus, device, 0);
        if !header.is_present() {
            continue;
        }
        pci_function_secbus_check(bus, device, 0, list);

        if header.is_multifunction() {
            for func in 1..8 {
                pci_function_secbus_check(bus, device, func, list);
            }
        }
    }
}

/// Performs a full PCI enumeration, by checking if every possible slot contains a device or not.
///
/// The `pci_enumerate_traversal` will be quicker if available, as it avoids checking for devices
/// that we know cannot be there.
pub fn pci_enumerate_all() -> PCIDevices {
    let mut list = Vec::new();
    for bus in 0..=255 {
        for device in 0..32 {
            pci_device_check(bus, device, &mut list);
        }
    }

    PCIDevices::from_devices(list)
}

/// Checks if a device is present, and enumerates its functions.
pub(super) fn pci_device_check(bus: u8, device: u8, list: &mut Vec<PCIDevice<'static>>) {
    let header = PCIHeader::read(bus, device, 0);
    if !header.is_present() {
        return;
    }
    list.push(PCIDevice::load(bus, device, 0));

    if header.is_multifunction() {
        for func in 1..8 {
            let header = PCIHeader::read(bus, device, func);
            if !header.is_present() {
                continue;
            }
            list.push(PCIDevice::load(bus, device, func));
        }
    }
}

/// Reads a `long` ([`u32`]) from the PCI Configuration Space.
pub fn pci_read_long(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let mut config_address: u32 = 0;

    config_address |= 0x80000000;
    config_address |= (bus as u32) << 16;
    config_address |= (device as u32) << 11;
    config_address |= (func as u32) << 8;

    // Reads must be 32-bits aligned, so we set the lowest 2 bits to zero, so that we access a
    // multiple of 4 bytes (a `long`, or `dword`).
    config_address |= ((offset as u32) << 2) & 0xfc;

    // `0xcf8` is the `CONFIG_ADDRESS` I/O port, used to specify the configuration address required
    // to be accessed.
    outl(0xcf8, config_address);

    // `0xcfc` is the `CONFIG_DATA` I/O port, it contains the data to transfert to or from the
    // `CONFIG_DATA` register.
    inl(0xcfc)
}

/// Writes a `long` ([`u32`]) to the PCI Configuration Space.
pub fn pci_write_long(bus: u8, device: u8, func: u8, offset: u8, data: u32) {
    let mut config_address: u32 = 0;

    config_address |= 0x80000000;
    config_address |= (bus as u32) << 16;
    config_address |= (device as u32) << 11;
    config_address |= (func as u32) << 8;

    // Writes must be 32-bits aligned, so we set the lowest 2 bits to zero, so that we access a
    // multiple of 4 bytes (a `long`, or `dword`).
    config_address |= ((offset as u32) << 2) & 0xfc;

    // `0xcf8` is the `CONFIG_ADDRESS` I/O port, used to specify the configuration address required
    // to be written.
    outl(0xcf8, config_address);

    // `0xcfc` is the `CONFIG_DATA` I/O port, it contains the data to transfert to or from the
    // `CONFIG_DATA` register.
    outl(0xcfc, data);
}
