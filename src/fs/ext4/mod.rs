use alloc::{string::String, vec::Vec};
use core::{mem, ptr, slice};

use bytemuck::{cast, from_bytes, try_cast, Pod, Zeroable};

use crate::fs::ext4::extent::Ext4RealBlkId;
use crate::fs::ext4::inode::InodeNumber;
use crate::fs::ext4::sb::{Ext4Superblock, IncompatibleFeatureSet};
use crate::{
    drivers::ahci::{get_sata_drive, SATA_DRIVES},
    error,
    errors::{CanFail, IOError},
    fs::{
        ext4::{
            bitmap::{BlockBitmap, InodeBitmap},
            dir::Ext4Directory,
            extent::{Ext4InodeRelBlkId, Ext4InodeRelBlkIdRange, ExtentTree},
            inode::{Inode, InodeFileMode, InodeFlags, InodeSize},
        },
        IOResult, PartFS,
    },
    info, println,
};
use crate::{errors::MountError, fs::FsFile};

pub(super) mod bitmap;
pub mod dir;
pub(crate) mod extent;
mod file;
pub(crate) mod inode;
mod sb;

pub const EXT4_SIGNATURE: u16 = 0xEF53;

pub const EXT4_CHKSUM_TYPE_CRC32: u8 = 0x1;

#[derive(Clone, Debug)]
pub struct Ext4Fs {
    drive_id: usize,
    partition_id: usize,
    superblock: Ext4Superblock,
    group_descriptors: Vec<GroupDescriptor>,
}

impl Ext4Fs {
    pub fn root_dir(&self) -> Ext4Directory {
        let root_inode = self.__read_inode(InodeNumber::ROOT_DIR).unwrap();

        Ext4Directory::from_inode(
            self.drive_id,
            self.partition_id,
            root_inode,
            InodeNumber::ROOT_DIR,
        )
        .unwrap()
    }

    pub fn identify(drive_id: usize, partition_data: u64) -> Result<bool, IOError> {
        let mut drive = get_sata_drive(drive_id).lock();

        let sb_start_lba = (1024 / drive.logical_sector_size() as u64) + partition_data;
        let sb_size_in_lba = mem::size_of::<Ext4Superblock>() as u32 / drive.logical_sector_size();

        let mut raw_sb = [0u8; mem::size_of::<Ext4Superblock>()];
        drive.read(sb_start_lba as u64, sb_size_in_lba as u16, &mut raw_sb)?;

        let sb = unsafe { ptr::read(raw_sb.as_ptr() as *mut Ext4Superblock) };

        Ok(sb.magic.is_valid())
    }

    pub fn mount(
        drive_id: usize,
        partition_id: usize,
        partition_data: u64,
    ) -> Result<Self, MountError> {
        let mut drive = get_sata_drive(drive_id).lock();

        let sb_start_lba = (1024 / drive.logical_sector_size() as u64) + partition_data;
        let sb_size_in_lba = mem::size_of::<Ext4Superblock>() as u32 / drive.logical_sector_size();

        let mut raw_sb = [0u8; mem::size_of::<Ext4Superblock>()];
        drive
            .read(sb_start_lba as u64, sb_size_in_lba as u16, &mut raw_sb)
            .map_err(|_| MountError::IOError)?;

        let sb = unsafe { ptr::read(raw_sb.as_ptr() as *mut Ext4Superblock) };
        let isize = sb.inode_size;

        if sb.checksum_type == EXT4_CHKSUM_TYPE_CRC32 && !sb.validate_chksum() {
            return Err(MountError::BadSuperblock);
        }

        if !sb.magic.is_valid() {
            return Err(MountError::BadSuperblock);
        }

        info!(
            "ext4-fs",
            "mounted ext4 filesystem on drive {drive_id} partition {partition_id}"
        );

        info!(
            "ext4-fs",
            "label = {}    inodes_count = {}    blk_count = {}    mmp = {}    opts = {}",
            String::from(sb.volume_name),
            sb.inodes_count,
            sb.blk_count(),
            sb.mmp_enabled(),
            String::from(sb.mount_opts)
        );

        let mut fs = Self {
            drive_id,
            partition_id,
            superblock: sb,
            group_descriptors: alloc::vec![],
        };
        drop(drive);

        fs.init_group_descriptors(partition_data)
            .map_err(|_| MountError::IOError)?;

        Ok(fs)
    }

    pub fn init_group_descriptors(&mut self, part_offset: u64) -> CanFail<IOError> {
        self.group_descriptors.clear();

        for i in 0..self.superblock.bg_count() {
            let gd = self.__read_bg_descriptor_preinit(i, part_offset)?;
            self.group_descriptors.push(gd);
        }

        Ok(())
    }

    fn __read_blk_preinit(
        &self,
        blk_id: Ext4RealBlkId,
        part_offset: u64,
        buffer: &mut [u8],
    ) -> CanFail<IOError> {
        if blk_id > self.superblock.blk_count() {
            return Err(IOError::InvalidCommand);
        }
        let mut drive = get_sata_drive(self.drive_id).lock();

        let sectors_count = self.superblock.blk_size() / drive.logical_sector_size() as u64;
        let start_lba = part_offset
            + (blk_id * self.superblock.blk_size()) / drive.logical_sector_size() as u64;

        drive.read(start_lba, sectors_count as u16, buffer)
    }

    fn __read_blk(&self, blk_id: Ext4RealBlkId, buffer: &mut [u8]) -> CanFail<IOError> {
        let drive = get_sata_drive(self.drive_id).lock();
        let partition_data = drive
            .partitions
            .get(self.partition_id)
            .ok_or(IOError::Unknown)?
            .start_lba();

        drop(drive);

        self.__read_blk_preinit(blk_id, partition_data, buffer)
    }

    fn __read_bg_descriptor_preinit(
        &self,
        bg_id: u64,
        part_offset: u64,
    ) -> Result<GroupDescriptor, IOError> {
        assert!(bg_id < self.superblock.bg_count());

        let descriptor_size = if self
            .superblock
            .feature_incompat
            .includes(IncompatibleFeatureSet::EXT4_FEATURE_INCOMPAT_64BIT)
        {
            64
        } else {
            32
        };

        let initial_blk_offset =
            if self.superblock.blk_size() == mem::size_of::<Ext4Superblock>() as u64 {
                2
            } else {
                1
            };

        let descriptor_per_block = self.superblock.blk_size() / descriptor_size;
        let desc_blk_id =
            initial_blk_offset + (bg_id * descriptor_size) / self.superblock.blk_size();
        let desc_idx_in_blk = bg_id % descriptor_per_block;

        let mut blk = alloc::vec![0; self.superblock.blk_size() as usize];
        self.__read_blk_preinit(Ext4RealBlkId::from(desc_blk_id), part_offset, &mut blk)?;

        let raw_bg_descriptor = &blk[((desc_idx_in_blk * descriptor_size) as usize)
            ..(((desc_idx_in_blk + 1) * descriptor_size) as usize)];

        if self
            .superblock
            .feature_incompat
            .includes(IncompatibleFeatureSet::EXT4_FEATURE_INCOMPAT_64BIT)
        {
            Ok(GroupDescriptor::Size64(unsafe {
                ptr::read(raw_bg_descriptor.as_ptr() as *const GroupDescriptor64)
            }))
        } else {
            Ok(GroupDescriptor::Size32(unsafe {
                ptr::read(raw_bg_descriptor.as_ptr() as *const GroupDescriptor32)
            }))
        }
    }

    fn __read_bg_descriptor(&self, bg_id: u64) -> Result<GroupDescriptor, IOError> {
        let drive = get_sata_drive(self.drive_id).lock();
        let part_offset = drive
            .partitions
            .get(self.partition_id)
            .ok_or(IOError::Unknown)?
            .start_lba();
        drop(drive);

        self.__read_bg_descriptor_preinit(bg_id, part_offset)
    }

    pub(crate) fn __read_inode(&self, inode_id: InodeNumber) -> Result<Inode, IOError> {
        let inode_blk_group = (inode_id - 1) / self.superblock.inodes_per_group;
        let inode_bg_idx = (inode_id - 1) % self.superblock.inodes_per_group;
        let inode_byte_idx = u64::from(inode_bg_idx) * u64::from(self.superblock.inode_size);

        let inode_blk_offset: u64 = inode_byte_idx / self.superblock.blk_size();

        let inode_bytes_idx_in_blk: u64 = inode_byte_idx % self.superblock.blk_size();

        let descriptor = self
            .group_descriptors
            .get(inode_blk_group as usize)
            .ok_or(IOError::Unknown)?;

        let mut raw_inode_blk = alloc::vec![0; self.superblock.blk_size() as usize];

        match descriptor {
            GroupDescriptor::Size32(desc) => self.__read_blk(
                Ext4RealBlkId::from(inode_blk_offset + u64::from(desc.inode_table_blk_addr())),
                &mut raw_inode_blk,
            ),
            GroupDescriptor::Size64(desc) => self.__read_blk(
                Ext4RealBlkId::from(inode_blk_offset + desc.inode_table_blk_addr()),
                &mut raw_inode_blk,
            ),
        }?;

        let raw_inode = &raw_inode_blk[(inode_bytes_idx_in_blk as usize)
            ..(inode_bytes_idx_in_blk + self.superblock.inode_size as u64) as usize];

        let mut filled_inode = alloc::vec![0u8; mem::size_of::<Inode>()];
        filled_inode[..raw_inode.len()].copy_from_slice(raw_inode);

        let inode: Inode = *from_bytes(&filled_inode);
        inode.validate_chksum(self.superblock.uuid, inode_id);

        Ok(inode)
    }
}

/// Each block group on the file system has a `GroupDescriptor` associated with it.
///
/// A `block group` is a logical grouping of contiguous block.
#[derive(Clone, Copy, Debug)]
pub enum GroupDescriptor {
    Size32(GroupDescriptor32),
    Size64(GroupDescriptor64),
}

/// 32-bit version of the [`GroupDescriptor`].
///
/// Used if [`EXT4_FEATURE_INCOMPAT_64BIT`] is clear.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct GroupDescriptor32 {
    /// 32-bit location of block bitmap
    bg_block_bitmap: u32,

    /// 32-bit location of inode bitmap
    bg_inode_bitmap: u32,

    /// 32-bit location of inode table
    bg_inode_table: u32,

    /// 16-bit free block count
    bg_free_blocks_count: u16,

    /// 16-bit free inode count
    bg_free_inodes_count: u16,

    /// 16-bit directory count
    bg_used_dirs_count: u16,

    /// Block group flags
    bg_flags: u16,

    /// 32-bit location of snapshot exclusion bitmap
    bg_exclude_bitmap_lo: u32,

    /// 16-bit of the block bitmap checksum
    bg_block_bitmap_csum_lo: u16,

    /// 16-bit of the inode bitmap checksum
    bg_inode_bitmap_csum_lo: u16,

    /// 16-bit unused inode count
    bg_itable_unused_lo: u16,

    /// Group descriptor checksum
    bg_checksum: u16,
}

impl GroupDescriptor32 {
    pub fn inode_table_blk_addr(&self) -> u32 {
        self.bg_inode_table
    }
}

/// 64-bit version of the [`GroupDescriptor`]
///
/// Used if [`EXT4_FEATURE_INCOMPAT_64BIT`] is set.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct GroupDescriptor64 {
    /// Lower 32-bit of location of block bitmap
    bg_block_bitmap_lo: u32,

    /// Lower 32-bit of location of inode bitmap
    bg_inode_bitmap_lo: u32,

    /// Lower 32-bit of location of inode table
    bg_inode_table_lo: u32,

    /// Lower 16-bit of free block count
    bg_free_blocks_count_lo: u16,

    /// Lower 16-bit of free inode count
    bg_free_inodes_count_lo: u16,

    /// Lower 16-bit of directory count
    bg_used_dirs_count_lo: u16,

    /// Block group flags
    bg_flags: u16,

    /// Lower 32-bit of location of snapshot exclusion bitmap
    bg_exclude_bitmap_lo: u32,

    /// Lower 16-bit of the block bitmap checksum
    bg_block_bitmap_csum_lo: u16,

    /// Lower 16-bit of the inode bitmap checksum
    bg_inode_bitmap_csum_lo: u16,

    /// Lower 16-bit of unused inode count
    bg_itable_unused_lo: u16,

    /// Group descriptor checksum
    bg_checksum: u16,

    /// High 32-bits of block bitmap
    bg_block_bitmap_hi: u32,

    /// High 32-bits of inode bitmap
    bg_inode_bitmap_hi: u32,

    /// High 32-bits of inode table
    bg_inode_table_hi: u32,

    /// High 16-bits of free blocks count
    bg_free_blocks_count_hi: u16,

    /// High 16-bits of free inodes count
    bg_free_inodes_count_hi: u16,

    /// High 16-bits of directory used count
    bg_used_dirs_count_hi: u16,

    /// High 16-bits of unused inode count
    bg_itable_unused_hi: u16,

    /// High 32-bits of location of snapshot exclusion bitmap
    bg_exclude_bitmap_hi: u32,

    /// High 16-bits of the block bitmap checksum
    bg_block_bitmap_csum_hi: u16,

    /// High 16_bits of the inode bitmap checksum
    bg_inode_bitmap_csum_hi: u16,
    reserved: u32,
}

impl GroupDescriptor64 {
    pub(crate) fn get_blk_bitmap(&self, fs: &Ext4Fs) -> BlockBitmap {
        let mut blk_bitmap_buf = alloc::vec![0; fs.superblock.blk_size() as usize];

        fs.__read_blk(
            Ext4RealBlkId::from(self.block_bitmap_blk_addr()),
            &mut blk_bitmap_buf,
        )
        .unwrap();
        let bitmap = BlockBitmap::from_bytes(
            &blk_bitmap_buf[..(fs.superblock.inodes_per_group / 8) as usize],
        );
        let chksum = u32::from(self.bg_block_bitmap_csum_lo)
            | (u32::from(self.bg_block_bitmap_csum_hi) << 16);
        bitmap.validate_chksum(fs.superblock.uuid, cast(chksum));

        bitmap
    }
    pub(crate) fn get_inode_bitmap(&self, fs: &Ext4Fs) -> InodeBitmap {
        let mut inode_bitmap_buf = alloc::vec![0; fs.superblock.blk_size() as usize];

        fs.__read_blk(
            Ext4RealBlkId::from(self.inode_bitmap_blk_addr()),
            &mut inode_bitmap_buf,
        )
        .unwrap();
        let bitmap = InodeBitmap::from_bytes(
            &inode_bitmap_buf[..(fs.superblock.inodes_per_group / 8) as usize],
        );
        let chksum = u32::from(self.bg_inode_bitmap_csum_lo)
            | (u32::from(self.bg_inode_bitmap_csum_hi) << 16);
        bitmap.validate_chksum(fs.superblock.uuid, cast(chksum));

        bitmap
    }

    pub fn block_bitmap_blk_addr(&self) -> u64 {
        self.bg_block_bitmap_lo as u64 | ((self.bg_block_bitmap_hi as u64) << 32)
    }

    pub fn inode_bitmap_blk_addr(&self) -> u64 {
        self.bg_inode_bitmap_lo as u64 | ((self.bg_inode_bitmap_hi as u64) << 32)
    }

    pub fn inode_table_blk_addr(&self) -> u64 {
        self.bg_inode_table_lo as u64 | ((self.bg_inode_table_hi as u64) << 32)
    }

    pub fn snapshot_excl_bitmap_blk_addr(&self) -> u64 {
        self.bg_exclude_bitmap_lo as u64 | ((self.bg_exclude_bitmap_hi as u64) << 32)
    }

    pub fn free_blk_count(&self) -> u32 {
        self.bg_free_blocks_count_lo as u32 | ((self.bg_free_blocks_count_hi as u32) << 16)
    }

    pub fn free_inode_count(&self) -> u32 {
        self.bg_free_inodes_count_lo as u32 | ((self.bg_free_inodes_count_hi as u32) << 16)
    }

    pub fn directory_count(&self) -> u32 {
        self.bg_used_dirs_count_lo as u32 | ((self.bg_used_dirs_count_hi as u32) << 16)
    }

    pub fn unused_inodes_count(&self) -> u32 {
        self.bg_itable_unused_lo as u32 | ((self.bg_itable_unused_hi as u32) << 16)
    }
}

/// Block group flag: Inode table and bitmap are not initialized
pub const EXT4_BG_INODE_UNINIT: u16 = 0x0001;

/// Block group flag: Block bitmap is not initialized
pub const EXT4_BG_BLOCK_UNINIT: u16 = 0x0002;

/// Block group flag: Inode table is zeroed
pub const EXT4_BG_INODE_ZEROED: u16 = 0x0004;

/*****************************************************************/
/*                                                               */
/* CRC LOOKUP TABLE                                              */
/* ================                                              */
/* The following CRC lookup table was generated automagically    */
/* by the Rocksoft^tm Model CRC Algorithm Table Generation       */
/* Program V1.0 using the following model parameters:            */
/*                                                               */
/*    Width   : 4 bytes.                                         */
/*    Poly    : 0x1EDC6F41L                                      */
/*    Reverse : TRUE.                                            */
/*                                                               */
/* For more information on the Rocksoft^tm Model CRC Algorithm,  */
/* see the document titled "A Painless Guide to CRC Error        */
/* Detection Algorithms" by Ross Williams                        */
/* (ross@guest.adelaide.edu.au.). This document is likely to be  */
/* in the FTP archive "ftp.adelaide.edu.au/pub/rocksoft".        */
/*                                                               */
/*****************************************************************/

const CRC32C_LO_TABLE: [u32; 256] = [
    0x00000000, 0xF26B8303, 0xE13B70F7, 0x1350F3F4, 0xC79A971F, 0x35F1141C, 0x26A1E7E8, 0xD4CA64EB,
    0x8AD958CF, 0x78B2DBCC, 0x6BE22838, 0x9989AB3B, 0x4D43CFD0, 0xBF284CD3, 0xAC78BF27, 0x5E133C24,
    0x105EC76F, 0xE235446C, 0xF165B798, 0x030E349B, 0xD7C45070, 0x25AFD373, 0x36FF2087, 0xC494A384,
    0x9A879FA0, 0x68EC1CA3, 0x7BBCEF57, 0x89D76C54, 0x5D1D08BF, 0xAF768BBC, 0xBC267848, 0x4E4DFB4B,
    0x20BD8EDE, 0xD2D60DDD, 0xC186FE29, 0x33ED7D2A, 0xE72719C1, 0x154C9AC2, 0x061C6936, 0xF477EA35,
    0xAA64D611, 0x580F5512, 0x4B5FA6E6, 0xB93425E5, 0x6DFE410E, 0x9F95C20D, 0x8CC531F9, 0x7EAEB2FA,
    0x30E349B1, 0xC288CAB2, 0xD1D83946, 0x23B3BA45, 0xF779DEAE, 0x05125DAD, 0x1642AE59, 0xE4292D5A,
    0xBA3A117E, 0x4851927D, 0x5B016189, 0xA96AE28A, 0x7DA08661, 0x8FCB0562, 0x9C9BF696, 0x6EF07595,
    0x417B1DBC, 0xB3109EBF, 0xA0406D4B, 0x522BEE48, 0x86E18AA3, 0x748A09A0, 0x67DAFA54, 0x95B17957,
    0xCBA24573, 0x39C9C670, 0x2A993584, 0xD8F2B687, 0x0C38D26C, 0xFE53516F, 0xED03A29B, 0x1F682198,
    0x5125DAD3, 0xA34E59D0, 0xB01EAA24, 0x42752927, 0x96BF4DCC, 0x64D4CECF, 0x77843D3B, 0x85EFBE38,
    0xDBFC821C, 0x2997011F, 0x3AC7F2EB, 0xC8AC71E8, 0x1C661503, 0xEE0D9600, 0xFD5D65F4, 0x0F36E6F7,
    0x61C69362, 0x93AD1061, 0x80FDE395, 0x72966096, 0xA65C047D, 0x5437877E, 0x4767748A, 0xB50CF789,
    0xEB1FCBAD, 0x197448AE, 0x0A24BB5A, 0xF84F3859, 0x2C855CB2, 0xDEEEDFB1, 0xCDBE2C45, 0x3FD5AF46,
    0x7198540D, 0x83F3D70E, 0x90A324FA, 0x62C8A7F9, 0xB602C312, 0x44694011, 0x5739B3E5, 0xA55230E6,
    0xFB410CC2, 0x092A8FC1, 0x1A7A7C35, 0xE811FF36, 0x3CDB9BDD, 0xCEB018DE, 0xDDE0EB2A, 0x2F8B6829,
    0x82F63B78, 0x709DB87B, 0x63CD4B8F, 0x91A6C88C, 0x456CAC67, 0xB7072F64, 0xA457DC90, 0x563C5F93,
    0x082F63B7, 0xFA44E0B4, 0xE9141340, 0x1B7F9043, 0xCFB5F4A8, 0x3DDE77AB, 0x2E8E845F, 0xDCE5075C,
    0x92A8FC17, 0x60C37F14, 0x73938CE0, 0x81F80FE3, 0x55326B08, 0xA759E80B, 0xB4091BFF, 0x466298FC,
    0x1871A4D8, 0xEA1A27DB, 0xF94AD42F, 0x0B21572C, 0xDFEB33C7, 0x2D80B0C4, 0x3ED04330, 0xCCBBC033,
    0xA24BB5A6, 0x502036A5, 0x4370C551, 0xB11B4652, 0x65D122B9, 0x97BAA1BA, 0x84EA524E, 0x7681D14D,
    0x2892ED69, 0xDAF96E6A, 0xC9A99D9E, 0x3BC21E9D, 0xEF087A76, 0x1D63F975, 0x0E330A81, 0xFC588982,
    0xB21572C9, 0x407EF1CA, 0x532E023E, 0xA145813D, 0x758FE5D6, 0x87E466D5, 0x94B49521, 0x66DF1622,
    0x38CC2A06, 0xCAA7A905, 0xD9F75AF1, 0x2B9CD9F2, 0xFF56BD19, 0x0D3D3E1A, 0x1E6DCDEE, 0xEC064EED,
    0xC38D26C4, 0x31E6A5C7, 0x22B65633, 0xD0DDD530, 0x0417B1DB, 0xF67C32D8, 0xE52CC12C, 0x1747422F,
    0x49547E0B, 0xBB3FFD08, 0xA86F0EFC, 0x5A048DFF, 0x8ECEE914, 0x7CA56A17, 0x6FF599E3, 0x9D9E1AE0,
    0xD3D3E1AB, 0x21B862A8, 0x32E8915C, 0xC083125F, 0x144976B4, 0xE622F5B7, 0xF5720643, 0x07198540,
    0x590AB964, 0xAB613A67, 0xB831C993, 0x4A5A4A90, 0x9E902E7B, 0x6CFBAD78, 0x7FAB5E8C, 0x8DC0DD8F,
    0xE330A81A, 0x115B2B19, 0x020BD8ED, 0xF0605BEE, 0x24AA3F05, 0xD6C1BC06, 0xC5914FF2, 0x37FACCF1,
    0x69E9F0D5, 0x9B8273D6, 0x88D28022, 0x7AB90321, 0xAE7367CA, 0x5C18E4C9, 0x4F48173D, 0xBD23943E,
    0xF36E6F75, 0x0105EC76, 0x12551F82, 0xE03E9C81, 0x34F4F86A, 0xC69F7B69, 0xD5CF889D, 0x27A40B9E,
    0x79B737BA, 0x8BDCB4B9, 0x988C474D, 0x6AE7C44E, 0xBE2DA0A5, 0x4C4623A6, 0x5F16D052, 0xAD7D5351,
];

fn crc32c_calc(buf: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFF;

    for &b in buf {
        crc = CRC32C_LO_TABLE[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }

    crc
}
