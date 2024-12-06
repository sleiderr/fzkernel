pub mod acpi;
#[cfg(feature = "alloc")]
pub mod ahci;
#[cfg(feature = "alloc")]
pub mod ide;
#[cfg(feature = "alloc")]
pub mod pci;
pub mod ps2;

#[cfg(feature = "alloc")]
pub mod generics;
