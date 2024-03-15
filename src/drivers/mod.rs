pub mod acpi;
#[cfg(feature = "alloc")]
pub mod ahci;
#[cfg(feature = "alloc")]
pub mod ide;
#[cfg(feature = "alloc")]
pub mod pci;

#[cfg(feature = "alloc")]
pub mod generics;
