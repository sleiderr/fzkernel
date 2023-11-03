use crate::io::pic::PIC;
use core::arch::asm;
use fzproc_macros::{interrupt, interrupt_default};

/// This module defines every interrupts referenced by the IDT
///
/// It provides several utilities to define interrupts.
///
/// To define a simple interrupt, precede your fn definition with the proc
/// macro #\[interrupt].
/// This procedural macro will wrap your routine in a naked rust function
/// allowing custom prologue and epilogue to handle `icall` CPU instruction
///
/// # Examples:
///
/// ```
/// #[interrupt]
/// pub fn _int0x00() {
///     do_something()
/// }
/// ```
///
/// The naming convention implies that interrupt are named as follows :
///
/// ```
/// format!("_int0x{:x}", int_number)
/// ```
///
/// In order to define a default template for interrupt that you don't want to define or
/// that you haven't implemented yet, [`fzproc_macros`] provides a proc_macro #\[interrupt_default]
/// This one will provide a generic variable :
///
/// ```
/// let int_code : usize
/// ```
///
/// usable in your default template to perform generic handle actions at runtime.
///
/// ```
/// #[interrupt_default]
/// pub fn _int_default(){
///      print!("{}", int_code);
/// }
/// ```
///
/// This would typically print the interrupt number for each interrupt triggered
/// The default function should always be called _int_default and is required.
///
/// To import [`crate::interrupts`] you will have to call a proc macro in front of your
/// declaration in order to auto generate the IDT from your handlers module.
/// The proc macro takes an offset as an arg to chose where you want to write the table.
///
/// e.g. This example shows how you would save the idt at offset 0x8
///
/// ```
/// #[interrupt_descriptor_table(0x8)]
/// mod interrupts;
/// ```
///
/// Loading of the table will then be achieved by calling the following function
/// from your main function.
///
/// ```
/// generate_idt()
/// ```

//A static CONTROLLER
const CONTROLLER: PIC = PIC {
    master_cmd_port: 0x20,
    master_data_port: 0x21,
    slave_cmd_port: 0xA0,
    slave_data_port: 0xA1,
};

#[interrupt]
pub fn _int0x73() {
    crate::drivers::ahci::irq_entry();
}

#[interrupt_default]
pub fn _int_default() {
    CONTROLLER.acknowledge_master();
    CONTROLLER.acknowledge_slave();
}
