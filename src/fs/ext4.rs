use crate::io::disk::bios::AddressPacket;
use core::mem::transmute;
use core::ptr::read_volatile;

#[repr(C, packed)]
pub struct Superblock {
    pub s_inodes_count: u32,
    pub s_blocks_count: u32,
    pub s_r_blocks_count: u32,
    pub s_free_blocks_count: u32,
    pub s_free_inodes_count: u32,
    pub s_first_datablock: u32,
    pub s_log_block_size: u32,
    pub s_log_frag_size: u32,
    pub s_blocks_per_group: u32,
    pub s_frags_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16, //0xEF53
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_def_resuid: u16,
    pub s_def_resgid: u16,
    pub s_first_ino: u32,
    pub s_inode_size: u16,
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u8; 16],
    pub s_volume_name: [u8; 16],
    pub s_last_mounted: [u8; 64],
    pub s_algo_bitmap: u32,
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_block: u8,
    pub s_journal_uuid: [u8; 16],
    pub s_journal_inum: u32,
    pub s_journal_dev: u32,
    pub s_last_orphan: u32,
    pub s_hash_seed: [u32; 4],
    pub s_def_hash_version: u8,
    pub s_default_mount_options: u32,
    pub s_first_meta_bg: u32,
    pub s_mkfs_time: u32,
    pub s_jnl_blocks: [u32; 17],
    pub s_blocks_count_hi: u32,
    pub s_r_blocks_count_hi: u32,
    pub s_free_blocks_count_hi: u32,
    pub s_min_extra_isize: u16,
    pub s_want_extra_isize: u16,
    pub s_flags: u32,
    pub s_raid_stride: u16,
    pub s_mmp_interval: u16,
    pub s_mmp_block: u64,
    pub s_raid_stripe_width: u32,
    pub s_log_groups_per_flex: u8,
    pub s_checksum_type: u8,
    pub s_reserved_pad: u16,
    pub s_kbytes_written: u64,
    pub s_snapshot_inum: u32,
    pub s_snapshot_id: u32,
    pub s_snapshot_r_blocks_count: u64,
    s_snapshot_list: u32,
    s_error_count: u32,
    s_first_error_time: u32,
    s_first_error_ino: u32,
    s_first_error_block: u64,
    s_first_error_func: [u8; 32],
    s_first_error_line: u32,
    s_last_error_time: u32,
    s_last_error_ino: u32,
    s_last_error_line: u32,
    s_last_error_block: u64,
    s_last_error_func: [u8; 32],
    s_mount_opts: [u8; 64],
    s_usr_quota_inum: u32,
    s_grp_quota_inum: u32,
    s_overhead_blocks: u32,
    s_backup_bgs: [u32; 2],
    s_encrypt_algos: [u8; 4],
    s_encrypt_pw_salt: [u8; 16],
    s_lpf_ino: u32,
    s_prj_quota_inum: u32,
    s_checksum_seed: u32,
    s_reserved: [u32; 98],
    s_checksum: u32,
}

impl Superblock {
    pub fn list_root(&self) {}

    pub fn load_block(&self, n: u32, partition: &Ext4Partition, buffer: u32) -> Result<(), ()> {
        let block_size_bytes = 2u32.pow((10 + self.s_log_block_size)) as u32;

        partition.read(n * block_size_bytes, block_size_bytes, buffer)
    }

    // Returns a reference to an Inode given its number (assuming default inode record size is 256 bytes)
    pub fn get_inode(&mut self, inode_nb: u32, partition: &Ext4Partition) -> &Inode {
        let block_group = (inode_nb - 1) / self.s_inodes_per_group;
        let index = (inode_nb - 1) % self.s_inodes_per_group;
        let block_size = 2u32.pow((10 + self.s_log_block_size)) as u32;
        let grp_descriptor_addr = block_size + 64 * block_group;

        partition.read(grp_descriptor_addr, 4096, 0x1500);

        let grp_descriptor_addr =
            (0x1500 + grp_descriptor_addr % 512) as *mut BlockGroupDescriptor32;
        let grp_desc: &BlockGroupDescriptor32;
        grp_desc = unsafe { transmute(grp_descriptor_addr) };

        if self.s_inode_size == 0 {
            self.s_inode_size = 256
        }

        let inode_table_address = grp_desc.bg_inode_table * block_size;
        let inode_address = inode_table_address + (self.s_inode_size as u32) * index;

        partition.read(inode_address, 512, 0x1500);

        let inode: &Inode;
        let inode_addr = (0x1500 + inode_address % 512) as *mut Inode;

        inode = unsafe { transmute(inode_addr) };

        inode
    }
}

#[repr(C, packed)]
struct BlockGroupDescriptor32 {
    bg_block_bitmap: u32,
    bg_inode_bitmap: u32,
    bg_inode_table: u32,
    bg_free_blocks_count: u16,
    bg_free_inodes_count: u16,
    bg_used_dirs_count: u16,
    bg_flags: u16,
    bg_exclude_bitmap_lo: u32,
    bg_block_bitmap_csum_lo: u16,
    bg_inode_bitmap_csum_lo: u16,
    bg_itable_unused_lo: u16,
    bg_checksum: u16,
    bg_reserved: [u8; 32],
}

#[repr(C, packed)]
pub struct Inode {
    pub i_mode: u16,
    i_uid: u16,
    i_size: u32,
    i_atime: u32,
    i_ctime: u32,
    i_mtime: u32,
    i_dtime: u32,
    i_gid: u16,
    i_links_count: u16,
    pub i_blocks: u32,
    i_flags: u32,
    i_osd1: u32,
    pub i_block: [u32; 15],
    i_generation: u32,
    i_file_acl: u32,
    i_dir_acl: u32,
    i_faddr: u32,
    i_osd2: [u8; 12],
    i_extra_isize: u16,
    i_checksum_hi: u16,
    i_ctime_extra: u32,
    i_mtime_extra: u32,
    i_atime_extra: u32,
    i_crtime: u32,
    i_crtime_extra: u32,
    i_version_hi: u32,
    i_projid: u32,
}

#[repr(C, packed)]
struct Ext4ExtentHeader {
    eh_magic: u16,
    eh_entries: u16,
    eh_max: u16,
    eh_depth: u16,
    eh_generation: u32,
}

#[repr(C, packed)]
struct Ext4Extent {
    ee_block: u32,
    ee_len: u16,
    ee_start_hi: u16,
    ee_start_lo: u32,
}

#[repr(C, packed)]
struct Ext4ExtentIdx {
    ei_block: u32,
    ei_leaf_lo: u32,
    ei_leaf_hi: u16,
    ei_unused: u16,
}

impl Inode {
    // Returns true is this inode uses an extent tree
    pub fn uses_extent_tree(&self) -> bool {
        return self.i_flags == 0x80000;
    }

    // Get block number of the nth data block.
    pub fn get_nth_data_block(&self, block_size: u32, n: u32, partition: &Ext4Partition) -> u64 {
        let q = self.i_size as i32 / block_size as i32;
        let r = self.i_size as i32 % block_size as i32;
        if !((n <= q as u32) | ((n == (q as u32 + 1)) & (r > 0))) {
            return 0;
        }
        if self.uses_extent_tree() {
            return self.get_nth_data_block_extent(block_size, n, partition);
        } else {
            return 0;
        }
    }

    // Get block number of the nth data block, considering this inode uses an extent tree structure
    pub fn get_nth_data_block_extent(
        &self,
        block_size: u32,
        n: u32,
        partition: &Ext4Partition,
    ) -> u64 {
        let mem_offset = self as *const Inode as u32 + 0x28;
        self.explore_next_layer(mem_offset, n, block_size, partition)
    }

    // Explore next layer, used by get_nth_data_block_extent
    pub fn explore_next_layer(
        &self,
        mem_offset: u32,
        n: u32,
        block_size: u32,
        partition: &Ext4Partition,
    ) -> u64 {
        let header_address = mem_offset as *const Ext4ExtentHeader;
        let header: &Ext4ExtentHeader;
        header = unsafe { transmute(header_address) };
        let leaf_number = header.eh_entries;
        // Handle leaves
        if header.eh_depth == 0 {
            for i in 0..leaf_number {
                let extent_addr = (mem_offset + 12 * (i + 1) as u32) as *const Ext4Extent;
                let extent: &Ext4Extent;
                extent = unsafe { transmute(extent_addr) };
                let len = {
                    if extent.ee_len <= 32768 {
                        extent.ee_len
                    } else {
                        extent.ee_len - 32768
                    }
                };
                if extent.ee_block + len as u32 >= n {
                    // Return address of n-th block
                    return (extent.ee_start_lo as u64 + ((extent.ee_start_hi as u64) << 32))
                        + (n - extent.ee_block) as u64;
                }
            }
            return 0;
        } else {
            // Offset + block_size in memory and iterate over entries recursively
            for i in 0..leaf_number {
                let extent_idx: &Ext4ExtentIdx;
                let extent_idx_addr = (mem_offset + 12 * (i + 1) as u32) as *const Ext4ExtentIdx;
                extent_idx = unsafe { transmute(extent_idx_addr) };
                let next_block_address =
                    extent_idx.ei_leaf_lo as u64 + (extent_idx.ei_leaf_hi as u64) << 2;
                // let a = AddressPacket::new((block_size / 512) as u16, mem_offset + block_size, (partition_offset + next_block_address / 512) as u64);
                let _ = partition.read(
                    next_block_address as u32,
                    block_size,
                    mem_offset + block_size,
                );
                return self.explore_next_layer(mem_offset + block_size, n, block_size, partition);
            }
            return 0;
        }
    }

    // Parse an inode as a directory, using at most 2 * block_size of memory
    pub fn parse_as_directory(&self, offset: u32, part: &Ext4Partition, block_size: u32) {
        let mut current_block = 0;
        let first_block = self.get_nth_data_block_extent(block_size, current_block, part);
        //debug!(first_block);

        let result = part.read((first_block as u32) * block_size, block_size, offset);
        let mut parser = offset;
        let mut inode = unsafe { read_volatile(offset as *const u32) };
        // The end is defined by a 0x00 inode pointer
        while inode != 0x00 {
            let mut flag_reset = false;
            let mut begin = parser;
            parser += 4;
            let rec_len = unsafe { read_volatile(parser as *const u16) };
            // Test if we need to load the next block
            if rec_len as u32 + parser - offset >= block_size {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                flag_reset = true;
            }
            parser += 2;
            let name_len = unsafe { read_volatile(parser as *const u8) };
            parser += 1;
            let type_flag = unsafe { read_volatile(parser as *const u8) };
            parser += 1;
            for i in 0..name_len {
                let char = unsafe { read_volatile(parser as *const u8) };
                parser += 1;
            }
            parser = begin + rec_len as u32;

            inode = unsafe { read_volatile(parser as *const u32) };

            // If reset flag is set, return to offset for the next block
            if flag_reset {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                parser = parser % block_size + offset;
                current_block += 1;
            }
        }
    }

    // Copy n block of file to a specific offset. This is risky as this function will overwrite everything it can.
    pub fn block_copy(&self, offset: u32, part: &Ext4Partition, block_size: u32, n: u32) {
        {
            let mut current_block = 0;
            let mut next_block = self.get_nth_data_block(block_size, current_block, part);
            let result = part.read((next_block as u32) * block_size, block_size, offset);
            while current_block < n {
                current_block += 1;
                next_block = self.get_nth_data_block(block_size, current_block, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + current_block * block_size,
                );
            }
        }
    }

    // Show content of a file using at most 1 block_size space in memory
    pub fn read_as_file(&self, offset: u32, part: &Ext4Partition, block_size: u32, n: u32) -> u8 {
        let mut total_byte = 0u32;
        let mut byte = 0u32;
        let mut current_block = 0;
        let mut next_block = self.get_nth_data_block(block_size, current_block, part);
        let result = part.read((next_block as u32) * block_size, block_size, offset);

        while total_byte < n {
            let char = unsafe { read_volatile((byte + offset) as *const u8) };
            if (byte % block_size == 0) & (byte != 0) {
                current_block += 1;
                next_block = self.get_nth_data_block(block_size, current_block, part);
                let result = part.read((next_block as u32) * block_size, block_size, offset);
                match result {
                    Err(_) => return 1,
                    Ok(_) => (),
                }
                byte = 0;
            } else {
                byte += 1;
            }
            total_byte += 1;
        }
        0
    }

    // Search in an inode considered as a directory, using at most 2 * block_size of memory
    pub fn search(
        &self,
        offset: u32,
        part: &Ext4Partition,
        block_size: u32,
        file_type: u8,
        name: &str,
    ) -> u32 {
        let name = name.as_bytes();
        let mut current_block = 0;
        let first_block = self.get_nth_data_block_extent(block_size, current_block, part);
        let result = part.read((first_block as u32) * block_size, block_size, offset);
        let mut parser = offset;
        let mut inode = unsafe { read_volatile(offset as *const u32) };
        // The end is defined by a 0x00 inode pointer
        while inode != 0x00 {
            let mut flag_reset = false;
            let mut begin = parser;
            parser += 4;

            let rec_len = unsafe { read_volatile(parser as *const u16) };

            // Test if we need to load the next block
            if rec_len as u32 + parser - offset >= block_size {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                flag_reset = true;
            }
            parser += 2;
            let name_len = unsafe { read_volatile(parser as *const u8) };
            parser += 1;
            let type_flag = unsafe { read_volatile(parser as *const u8) };

            let research_size = name.len();

            parser += 1;

            let mut same = 0;
            for i in 0..name_len {
                let char = unsafe { read_volatile(parser as *const u8) };
                if i < research_size as u8 {
                    if char == name[i as usize] {
                        same += 1;
                        parser += 1;
                    } else {
                        parser += 1;
                    }
                } else {
                    parser += 1
                }
            }

            parser = begin + rec_len as u32;

            if same != name.len() as u32 {
            } else if type_flag == file_type {
                return inode;
            }

            inode = unsafe { read_volatile(parser as *const u32) };

            // If reset flag is set, return to offset for the next block
            if flag_reset {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                parser = parser % block_size + offset;
                current_block += 1;
            }
        }

        return 0;
    }

    // Get block number of the nth data block, considering this inode uses a direct/indirect block addressing system
    pub fn get_nth_data_block_basic(
        &self,
        n: u32,
        offset: u32,
        block_size: u32,
        partition: &Ext4Partition,
    ) -> u32 {
        if n > 12 {
            let (path, depth) = self.get_path_recursive(n - 12, 1, block_size);
            let initial_block = self.i_block[(11 + depth) as usize];
            partition.read(initial_block * block_size, block_size, offset);
            let mut next_block_nb = 0u32;
            for step in path {
                if step != 0 {
                    next_block_nb = unsafe { read_volatile((offset + step * 4) as *const u32) };
                    partition.read(next_block_nb * block_size, block_size, offset);
                } else {
                    return next_block_nb;
                }
            }
            return 0;
        } else {
            return self.i_block[n as usize];
        }
    }

    pub fn get_path_recursive(&self, mut n: u32, depth: usize, block_size: u32) -> ([u32; 4], u8) {
        // Compute the number of bytes contained
        let address_per_block = (block_size / 4);
        let block_count = address_per_block.pow(depth as u32);

        // Check if we have to go to the next stage
        if n > block_count {
            return self.get_path_recursive(n - block_count, depth + 1, block_size);
        } else {
            let mut path = [0u32; 4];
            let mut i = 0;
            while n > block_count {
                path[i] = (n / (address_per_block).pow((depth - i - 1) as u32)) as u32;
                n = n % (address_per_block).pow((depth - i - 1) as u32);
                i += 1;
            }
            return (path, depth as u8);
        }
    }
}

pub struct Ext4Partition {
    pub offset: u32,
    pub drive: u8,
}

impl Ext4Partition {
    #[inline(never)]
    pub fn read(&self, offset: u32, length: u32, buffer: u32) -> Result<(), ()> {
        let offset = (offset / 512 + self.offset) as u64;
        let address = AddressPacket::new(
            (length / 512) as u16,
            (buffer >> 16) as u16,
            (buffer & 0xffff) as u16,
            offset,
        );
        address.disk_read(self.drive);
        Ok(())
    }
}

#[repr(C, packed)]
struct LinkedDirectoryEntry {
    inode: u16,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
    name: u32,
}
