//! This module provides utilities to control
//! and configure PIC chip. This chips act as intermediates
//! between hardware interrupts (IRQs) and CPU by remapping those
//! to given CPU interrupts.
//! Usually there are 2 PICs configured as master/slave.
//! Slave interrupts are thus be redirected to the master through one single IRQ.

use crate::io::{io_delay, outb, IOPort};

/// Initialization is made by sending ICW (Initialization Command Words)
/// to both Master and Slave controllers.
///
/// 4 ICWs have to be sent.
///
///  - - -
/// | ICW1 |
///  - - -
///
/// ICW1 tells controllers to start the init protocol and sets a few
/// general settings (CMD port)
///
/// Bindings :
///
/// 0 0 0 1 x 0 x x
///         |   | | _ _ _ _ _ ICW4 present (1) or not (0)
///         |   | _ _ _ _ _ _ Single (1) or cascade mode (0)
///         | _ _ _ _ _ _ _ _ Triggered mode : level (1) or edge (0)
///
/// We typically use 0x11 for ICW1.
///
///
///  - - -
/// | ICW2 |
///  - - -
///
/// ICW2 sets the offset of the controller for the IRQ => Interrupts
/// translation. (DATA port)
///
/// Bindings :
///
/// x x x x x 0 0 0
///
/// Offset is always a multiple of 8, implying that lower 3 bits are null.
///
///  - - -
/// | ICW3 |
///  - - -
///
/// ICW3 tells how master and slave controllers are connected together. (DATA port)
///
/// Bindings :
///
/// Master
/// x x x x x x x x
/// | | | | | | | |
///  - - - - - - - - - - - Bitmask for pins that have a slave connected to it
///
/// Slave :
/// 0 0 0 0 0 x x x
///           | | |
///            - - - - - - Slave ID (pin number he is connected to)
///
///  - - -
/// | ICW4 |
///  - - -
///
/// ICW4 Set additional settings. (DATA port)
///
/// Bindings :
/// 0 0 0 x x x x 1
///       | | | |
///       | | |  - - - - -  EOI auto (1) or normal (0)
///       |  - - - - - - -  Buffering mode
///        - - - - - - - - Special mode nested (1) or not (0)
///
/// Usually you won't set any of this bits except if you have advanced knowledge.

//Most common IRQs
const DEFAULT_ICW1: u8 = 0b00010001;
const DEFAULT_MASTER_ICW3: u8 = 0b00000100;
const DEFAULT_SLAVE_ICW3: u8 = 0b00000010;
const DEFAULT_ICW4: u8 = 0b00000001;

/// Most of the time, you will have to talk to PICs to send them specific commands.
/// That's why OCWs are made for (OCW stands for Operation Control Word).
/// There are two OCWs :
///
///  - - -
/// | OCW1 |
///  - - -
///
/// OCW1 sets a bitmask for enabled/disabled IRQs. (DATA port)
///
/// Bindings :
/// x x x x x x x x
/// | | | | | | | |
///  - - - - - - - - - - - - IRQ State Bitmask masked (1) or unmasked (0)
///
///  - - -
/// | OCW2 |
///  - - -
///
/// OCW2 is used to send EOI to acknowledge an interrupt. (CMD port)
/// Acknowledging an interrupt is necessary as long as the PIC will stop
/// sending interrupts until latest IRQ has not been acknowledged by the CPU.
/// Note that both slave and master PIC have to be acknowledged for a slave interrupt.
///
/// Bindings :
/// x x x 0 0 x x x
/// | | |     | | |
/// | | |      - - - - - - - Interrupt level for specific commands
/// | |  - - - - - - - - - - Send EOI (1)
/// |  - - - - - - - - - - - Command specific (1) or non-specific (0)
///  - - - - - - - - - - - - Rotate priorities (1) or not (0)
///

const SIMPLE_ACKNOWLEDGMENT: u8 = 0b00010000;

/// A Struct representing a PIC chip
pub struct PIC {
    pub master_cmd_port: u16,
    pub master_data_port: u16,
    pub slave_cmd_port: u16,
    pub slave_data_port: u16,
}

impl PIC {
    /// Creates a `PIC` with default bus ports for master/slave config.
    pub fn default() -> Self {
        Self {
            master_cmd_port: 0x20,
            master_data_port: 0x21,
            slave_cmd_port: 0xA0,
            slave_data_port: 0xA1,
        }
    }

    /// Remap `PIC` IRQs by setting new offset vectors.
    pub fn remap(&self, master_offset: u8, slave_offset: u8) {
        // Start init sequence
        outb(self.master_cmd_port.into(), DEFAULT_ICW1);
        io_delay();
        outb(self.slave_cmd_port.into(), DEFAULT_ICW1);
        io_delay();

        // Set vector offset
        outb(self.master_data_port.into(), master_offset);
        io_delay();
        outb(self.slave_data_port.into(), slave_offset);
        io_delay();

        // Master PIC has slave at IRQ2
        outb(self.master_data_port.into(), DEFAULT_MASTER_ICW3);
        io_delay();
        outb(self.slave_data_port.into(), DEFAULT_SLAVE_ICW3);
        io_delay();

        //
        outb(self.master_data_port.into(), DEFAULT_ICW4);
        io_delay();
        outb(self.slave_data_port.into(), DEFAULT_ICW4);
    }

    // Mask utilities are used to enable/disable interrupts on a given controller

    /// Masks slave PIC with given bitmask.
    pub fn mask_slave(&self, bitmask: u8) {
        outb(self.slave_data_port.into(), bitmask);
    }

    /// Masks master PIC with given bitmask.
    pub fn mask_master(&self, bitmask: u8) {
        outb(self.master_data_port.into(), bitmask);
    }

    /// Acknowledges master
    pub fn acknowledge_master(&self) {
        outb(self.master_cmd_port.into(), 0x20)
    }

    /// Acknowledges slave
    pub fn acknowledge_slave(&self) {
        outb(self.slave_cmd_port.into(), 0x20)
    }

    /// Acknowledges both master and slave
    pub fn acknowledge_all(&self) {
        self.acknowledge_master();
        self.acknowledge_slave();
    }
}
