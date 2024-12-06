pub mod headers;

/// Kernel loading related code.
pub mod fzkernel {
    use core::cmp::min;

    use alloc::format;
    use fzboot::kernel_syms::{KERNEL_LOAD_ADDR, KERNEL_SECTOR_SZ};
    use fzboot::x86::paging::bootinit_paging;
    use fzboot::{
        drivers::{
            generics::dev_disk::{get_sata_drive, sata_drives, DiskDevice},
            ide::AtaDeviceIdentifier,
        },
        info,
        mem::{MemoryAddress, PhyAddr},
        println,
    };

    /// Attempts to locate the partition containing the kernel code.
    /// Returns the drive and the partition id of the one on which the kernel is stored.
    ///
    /// Currently, only uses the name of the partition as a reference.
    pub fn locate_kernel_partition() -> (AtaDeviceIdentifier, usize) {
        let mut kernel_disk = AtaDeviceIdentifier::new(
            fzboot::drivers::generics::dev_disk::SataDeviceType::AHCI,
            0,
            0,
        );

        let mut kernel_part_id = 0;
        let mut found_kernel = false;

        for drive in sata_drives() {
            for (part_id, partition) in drive.partitions().iter().enumerate() {
                match partition.metadata() {
                    fzboot::fs::partitions::PartitionMetadata::MBR(mbr_part) => todo!(),
                    fzboot::fs::partitions::PartitionMetadata::GPT(gpt_part) => {
                        if gpt_part.name() == "kernelfs" {
                            kernel_disk = drive.identifier();
                            kernel_part_id = part_id;
                            found_kernel = true;

                            break;
                        }
                    }
                }
            }

            if found_kernel {
                info!(
                    "kernel",
                    "located kernel image ({}    partition_id = {})", kernel_disk, kernel_part_id
                );
                break;
            }
        }

        if !found_kernel {
            panic!("failed to locate kernel");
        }

        (kernel_disk, kernel_part_id)
    }

    /// Loads the kernel in memory from a disk device.
    pub fn load_kernel(device: AtaDeviceIdentifier, partition: usize) {
        let device = get_sata_drive(device).expect("could not find kernel disk device");
        let partition = device
            .partitions()
            .get(partition)
            .expect("could not find kernel partition");

        let mut sectors_read = 0;

        while sectors_read < KERNEL_SECTOR_SZ {
            let read = device.read(
                partition.start_lba() + u64::try_from(sectors_read).expect("invalid sectors count"),
                0x100,
            );
            let result = read.complete();
            let read_data = result.data.expect(
                format!(
                    "invalid data read when loading kernel (sector {})",
                    sectors_read
                )
                .as_str(),
            );

            unsafe {
                let mut mem_slice: &mut [u8] = core::slice::from_raw_parts_mut(
                    (KERNEL_LOAD_ADDR + sectors_read * 0x200).as_mut_ptr(),
                    min(0x200 * 0x200, read_data.len()),
                );
                mem_slice.copy_from_slice(&read_data);
            }

            sectors_read += 0x100;
        }

        info!(
            "kernel",
            "loaded kernel image to memory (base_addr = {}    virtual_base = {})",
            KERNEL_LOAD_ADDR,
            bootinit_paging::KERNEL_CODE_MAPPING_BASE
        );
    }
}
