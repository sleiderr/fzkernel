//! x86 paging address translation helpers.

use crate::mem::VirtAddr;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PageTableAddress(u16, u16, u16, u16);

impl PageTableAddress {
    pub(crate) fn pml4_offset(self) -> u16 {
        self.0
    }

    pub(crate) fn pdpte_offset(self) -> u16 {
        self.1
    }

    pub(crate) fn pde_offset(self) -> u16 {
        self.2
    }

    pub(crate) fn pte_offset(self) -> u16 {
        self.3
    }
}

pub struct PageAddressTranslator {}

impl Translator for PageAddressTranslator {
    fn translate_address(virt_addr: VirtAddr) -> PageTableAddress {
        let pml4_off = (u64::from(virt_addr) >> 39) & 0x1FF;
        let dir_ptr_off = (u64::from(virt_addr) >> 30) & 0x1FF;
        let dir_off = (u64::from(virt_addr) >> 21) & 0x1FF;
        let table_off = (u64::from(virt_addr) >> 12) & 0x1FF;

        PageTableAddress(
            u16::try_from(pml4_off).expect("infaillible conversion"),
            u16::try_from(dir_ptr_off).expect("infaillible conversion"),
            u16::try_from(dir_off).expect("infaillible conversion"),
            u16::try_from(table_off).expect("infaillible conversion"),
        )
    }
}

pub trait Translator {
    fn translate_address(virt_addr: VirtAddr) -> PageTableAddress;
}
