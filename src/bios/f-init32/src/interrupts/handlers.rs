use alloc::boxed::Box;
use alloc::format;
use core::arch::asm;
use core::mem::transmute;
use f_macros::{interrupt, interrupt_default};
use flib::int::scheduler::{IntScheduler, ScheduledAction};
use flib::{println, scheduler_ref};

/// This module defines every interrupts referenced by the IDT
/// It provides several utilities to define interrupts.
/// To define a simple interrupt, precede your fn definiton with the proc
/// macro #\[interrupt]
/// # Example
/// ```
/// #[interrupt]
/// pub fn _int0x00() {
///     do_something()
/// }
/// ```
/// The naming convention implies that interrupt are named as follows :
/// ```
/// format!("_int{:x}", int_number)
/// ```
///
/// In order to define a default template for interrupt that you don't want to define or
/// that you haven't implemented yet, [`f_macros`] provides a proc_macro #\[interrupt_default]
/// This one will provide a generic variable :
/// ```
/// let int_code : usize
/// ```
/// usable in your default template to perform generic handle actions at runtime.
///```
///#[interrupt_default]
///pub fn _int_default(){
///     print!("{}", int_code);
///}
///```
/// This would typically print the interrupt number for each interrupt triggered
/// The default function should always be called _int_default and is required.
///
/// To import [crate::interrupts] you will have to call a proc macro in front of your
/// declaration in order to auto generate the IDT from your handlers module.
/// The proc macro takes an offset as an arg to chose where you want to write the table.
/// e.g. This example shows how you would save the idt at offset 0x8
///```
///#[interrupt_descriptor_table(0x8)]
///mod interrupts;
///```
/// Loading of the table will then be achieved by calling the following function
/// from your main function.
/// ```
/// generate_idt()
/// ```


/// We will write at a fixed memory address the address to a static mutable reference to a global [`IntScheduler`]
const SCHEDULER_ADDRESS: *mut &mut Box<IntScheduler> = 0x00 as _;


#[interrupt]
pub fn _int0xd() {
    print_str("Lol", 0);
    loop {

    }

    // scheduler_ref!(scheduler);
    // let action = ScheduledAction::new(0x00);
    // scheduler.schedule(action);
}

#[interrupt_default]
pub fn _int_default() {
    println!("{}", int_code);
}
