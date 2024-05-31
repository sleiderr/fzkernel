//! `ext4fs` (Fourth Extended Filesystem) `FrozenBoot`'s implementation.
//!
//! `ext4` is widely used, and serves as the default filesystem in many Linux distributions (such
//! as _Debian_ or _Ubuntu_). It is an evolution of the `ext2` and `ext3` filesystems, more advanced and feature-rich,
//! with:
//!
//! - **Journaling**
//! - **Extents**
//! - **Large filesystem support**
//!
//! This implementation only covers the basic features of the `ext4` filesystem for now, and can only mount such
//! filesystems read-only.

#![allow(clippy::copy_iterator)]

use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};
use core::cell::RefCell;
use core::mem;

use bytemuck::from_bytes;
use hashbrown::HashMap;

use spin::RwLock;

use crate::drivers::generics::dev_disk::{get_sata_drive, DiskDevice};
use crate::drivers::ide::AtaDeviceIdentifier;
use crate::errors::MountError;
use crate::fs::ext4::block_grp::{BlockGroupNumber, GroupDescriptorCache, LockedGroupDescriptor};
use crate::fs::ext4::extent::Ext4RealBlkId;
use crate::fs::ext4::inode::{
    InodeCache, InodeCacheRemovalPolicy, InodeNumber, LockedInode, LockedInodeStrongRef,
};
use crate::fs::ext4::sb::{Ext4ChksumAlgorithm, Ext4Superblock, LockedSuperblock, Superblock};
use crate::fs::{Directory, Fs};
use crate::{
    errors::{CanFail, IOError},
    fs::{
        ext4::{dir::Ext4Directory, extent::ExtentTree, inode::Ext4Inode},
        IOResult,
    },
    info,
};

pub(super) mod bitmap;
pub(crate) mod block_grp;
pub(crate) mod dir;
pub(crate) mod extent;
pub(crate) mod file;
pub(crate) mod inode;
pub(crate) mod sb;

/// Strong pointer to a locked [`Ext4Fs`] structure.
///
/// This is the only interface to access the underlying structure, and thus the main way to interact with a `ext4`
/// filesystem.
///
/// The [`Ext4Fs`] structure will remain allocated for as long as the filesystem is mounted (as a strong reference is
/// kept in the global filesystem register).
pub(super) type LockedExt4Fs = Arc<RwLock<Ext4Fs>>;

/// Weak pointer to a locked [`Ext4Fs`] structure.
///
/// The underlying allocation will remain accessible as long as the filesystem is mounted. However, the weak reference
/// count is not checked when unmounting a filesystem, and therefore holding such reference do not ensure that the
/// filesystem cannot be unmounted, contrary to a [`LockedExt4Fs`] reference.
pub(super) type WeakLockedExt4Fs = Weak<RwLock<Ext4Fs>>;

/// Internal representation of a `ext4` filesystem.
///
/// Holds the main data structures required for the operation of the filesystem:
///
/// - `ext4`'s Superblock
///
/// - Various caches (inode cache, block group descriptor)
///
/// This is the main (and only?) interface to properly interact with the filesystem.
///
/// This structure can only be accessed through a smart [`Arc`] pointer, the underlying allocation is guaranteed to
/// remain valid while the `ext4` filesystem is mounted.
#[derive(Debug)]
pub(crate) struct Ext4Fs {
    drive_id: AtaDeviceIdentifier,
    partition_id: usize,

    superblock: LockedSuperblock,

    descriptors_cache: RefCell<GroupDescriptorCache>,

    inode_cache: RefCell<InodeCache>,

    fs_ptr: Weak<RwLock<Self>>,
}

impl Ext4Fs {
    /// Returns a strong reference ([`LockedInodeStrongRef`]) to an inode.
    ///
    /// This ensures that the corresponding [`Inode`] structure remains allocated for at least the lifetime of that
    /// reference.
    pub(super) fn get_inode_strong(&self, inode_id: InodeNumber) -> Option<LockedInodeStrongRef> {
        let mut inode_cache = self.inode_cache.borrow_mut();
        inode_cache.load_cached_inode_or_insert(inode_id)
    }

    /// Returns a weak reference ([`LockedInode`]) to an inode.
    ///
    /// This does not ensure that the corresponding [`Inode`] structure remains allocated for the lifetime of reference,
    /// as it may be removed from cache before (cache removal may occur even if the weak reference count is not null).
    pub(super) fn get_inode(&self, inode_id: InodeNumber) -> Option<LockedInode> {
        let mut inode_cache = self.inode_cache.borrow_mut();
        Some(Arc::downgrade(
            &inode_cache.load_cached_inode_or_insert(inode_id)?,
        ))
    }

    /// Returns a weak reference ([`LockedInode`]) to an inode.
    ///
    /// This also verifies that the given [`InodeNumber`] is a valid inode identifier in the filesystem, and therefore
    /// may be useful when the caller suspects that this inode number does not correspond to a real entry, as that
    /// function will return faster if so.
    ///
    /// This does not ensure that the corresponding [`Inode`] structure remains allocated for the lifetime of reference,
    /// as it may be removed from cache before (cache removal may occur even if the weak reference count is not null).//
    pub(super) fn get_inode_checked(&self, inode_id: InodeNumber) -> Option<LockedInode> {
        let mut inode_cache = self.inode_cache.borrow_mut();
        let sb = self.superblock.read();
        let inode_bg = sb.get_inode_blk_group(inode_id);

        if inode_id > sb.inodes_count || inode_bg > sb.bg_count() {
            return None;
        }

        let locked_descriptor = self.get_group_descriptor(inode_bg)?;

        let mut descriptor = locked_descriptor.write();

        if !descriptor.get_or_load_inode_bitmap().inode_in_use(inode_id) {
            return None;
        }

        Some(Arc::downgrade(
            &inode_cache.load_cached_inode_or_insert(inode_id)?,
        ))
    }

    /// Returns a strong reference ([`LockedGroupDescriptor`]) to a block group descriptor.
    pub(super) fn get_group_descriptor(
        &self,
        bg_number: BlockGroupNumber,
    ) -> Option<LockedGroupDescriptor> {
        let mut bg_desc_cache = self.descriptors_cache.borrow_mut();
        bg_desc_cache.load_cached_group_descriptor_or_insert(bg_number)
    }

    /// Returns the root directory of this filesystem.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. An error may mean that the filesystem
    /// is corrupted.
    pub(crate) fn root_dir(&self) -> IOResult<Directory> {
        Ok(Box::new(Ext4Directory::from_inode_id(
            self.fs_ptr.upgrade().ok_or(IOError::Unknown)?,
            InodeNumber::ROOT_DIR,
        )?))
    }

    /// Allocates a growable buffer (a [`Vec`]), initialized with a capacity corresponding to the block size
    /// of the filesystem.
    pub(crate) fn allocate_blk(&self) -> Vec<u8> {
        let sb = self.superblock.read();
        alloc::vec![0u8; usize::try_from(sb.blk_size()).expect("invalid block size")]
    }

    fn read_blk_from_device(&self, blk_id: Ext4RealBlkId, buffer: &mut [u8]) -> CanFail<IOError> {
        // With the new system for disk reads, this adds an unnecessary memcpy since the buffer is specified as an argument
        // Either change the way block reads are implemented in ext4, or add a way to read from disk and store the retrieved
        // bytes in a pre-specified buffer, as it was done previously.
        let sb = self.superblock.read();
        if blk_id > sb.blk_count() {
            return Err(IOError::InvalidCommand);
        }

        let mut drive = get_sata_drive(self.drive_id).ok_or(IOError::InvalidDevice)?;
        let partition_data = drive
            .partitions()
            .get(self.partition_id)
            .ok_or(IOError::Unknown)?
            .start_lba();

        let sectors_count = sb.blk_size() / drive.logical_sector_size();
        let start_lba = partition_data + (blk_id * sb.blk_size()) / drive.logical_sector_size();

        let read_req = drive
            .read(
                start_lba,
                u16::try_from(sectors_count).expect("invalid sectors count"),
            )
            .complete();

        buffer.copy_from_slice(&read_req.data.ok_or(IOError::Unknown)?);

        Ok(())
    }
}

impl Fs for Ext4Fs {
    fn mount(
        drive_id: AtaDeviceIdentifier,
        partition_id: usize,
        partition_data: u64,
    ) -> Result<LockedExt4Fs, MountError> {
        let mut drive = get_sata_drive(drive_id).ok_or(MountError::IOError)?;

        let sb_start_lba = (1024 / u64::from(drive.logical_sector_size())) + partition_data;
        let sb_size_in_lba = u32::try_from(mem::size_of::<Ext4Superblock>())
            .expect("invalid superblock size")
            / u32::try_from(drive.logical_sector_size()).expect("invalid logical sector size");

        let raw_sb = drive.read(
            sb_start_lba,
            u16::try_from(sb_size_in_lba).expect("invalid superblock size"),
        );

        let raw_sb_buffer = raw_sb.complete().data.ok_or(MountError::IOError)?;

        let ext4_sb = *from_bytes(&raw_sb_buffer);
        let sb = Superblock {
            ext4_superblock: ext4_sb,
        };

        if sb.checksum_type == Ext4ChksumAlgorithm::CHKSUM_CRC32_C && !sb.validate_chksum() {
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

        let fs = Arc::new_cyclic(|ptr| {
            RwLock::new(Ext4Fs {
                drive_id,
                partition_id,
                superblock: Arc::new(RwLock::new(sb)),
                inode_cache: RefCell::new(InodeCache {
                    hashtable: HashMap::default(),
                    removal_policy: InodeCacheRemovalPolicy::default(),
                    fs: ptr.clone(),
                }),
                fs_ptr: ptr.clone(),
                descriptors_cache: RefCell::new(GroupDescriptorCache {
                    descriptor_table: HashMap::default(),
                    fs: ptr.clone(),
                }),
            })
        });

        Ok(fs)
    }

    fn identify(drive_id: AtaDeviceIdentifier, partition_data: u64) -> Result<bool, IOError> {
        let mut drive = get_sata_drive(drive_id).ok_or(IOError::InvalidDevice)?;

        let sb_start_lba = (1024 / u64::from(drive.logical_sector_size())) + partition_data;
        let sb_size_in_lba = u32::try_from(mem::size_of::<Ext4Superblock>())
            .expect("invalid superblock size")
            / u32::try_from(drive.logical_sector_size()).expect("invalid logical sector size");

        let raw_sb = drive
            .read(
                sb_start_lba,
                u16::try_from(sb_size_in_lba).expect("invalid superblock size"),
            )
            .complete();

        let raw_sb_buffer = raw_sb.data.ok_or(IOError::Unknown)?;

        let ext4_sb: Ext4Superblock = *from_bytes(&raw_sb_buffer);

        Ok(ext4_sb.magic.is_valid())
    }
}

unsafe impl Sync for Ext4Fs {}

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
    0x0000_0000,
    0xF26B_8303,
    0xE13B_70F7,
    0x1350_F3F4,
    0xC79A_971F,
    0x35F1_141C,
    0x26A1_E7E8,
    0xD4CA_64EB,
    0x8AD9_58CF,
    0x78B2_DBCC,
    0x6BE2_2838,
    0x9989_AB3B,
    0x4D43_CFD0,
    0xBF28_4CD3,
    0xAC78_BF27,
    0x5E13_3C24,
    0x105E_C76F,
    0xE235_446C,
    0xF165_B798,
    0x030E_349B,
    0xD7C4_5070,
    0x25AF_D373,
    0x36FF_2087,
    0xC494_A384,
    0x9A87_9FA0,
    0x68EC_1CA3,
    0x7BBC_EF57,
    0x89D7_6C54,
    0x5D1D_08BF,
    0xAF76_8BBC,
    0xBC26_7848,
    0x4E4D_FB4B,
    0x20BD_8EDE,
    0xD2D6_0DDD,
    0xC186_FE29,
    0x33ED_7D2A,
    0xE727_19C1,
    0x154C_9AC2,
    0x061C_6936,
    0xF477_EA35,
    0xAA64_D611,
    0x580F_5512,
    0x4B5F_A6E6,
    0xB934_25E5,
    0x6DFE_410E,
    0x9F95_C20D,
    0x8CC5_31F9,
    0x7EAE_B2FA,
    0x30E3_49B1,
    0xC288_CAB2,
    0xD1D8_3946,
    0x23B3_BA45,
    0xF779_DEAE,
    0x0512_5DAD,
    0x1642_AE59,
    0xE429_2D5A,
    0xBA3A_117E,
    0x4851_927D,
    0x5B01_6189,
    0xA96A_E28A,
    0x7DA0_8661,
    0x8FCB_0562,
    0x9C9B_F696,
    0x6EF0_7595,
    0x417B_1DBC,
    0xB310_9EBF,
    0xA040_6D4B,
    0x522B_EE48,
    0x86E1_8AA3,
    0x748A_09A0,
    0x67DA_FA54,
    0x95B1_7957,
    0xCBA2_4573,
    0x39C9_C670,
    0x2A99_3584,
    0xD8F2_B687,
    0x0C38_D26C,
    0xFE53_516F,
    0xED03_A29B,
    0x1F68_2198,
    0x5125_DAD3,
    0xA34E_59D0,
    0xB01E_AA24,
    0x4275_2927,
    0x96BF_4DCC,
    0x64D4_CECF,
    0x7784_3D3B,
    0x85EF_BE38,
    0xDBFC_821C,
    0x2997_011F,
    0x3AC7_F2EB,
    0xC8AC_71E8,
    0x1C66_1503,
    0xEE0D_9600,
    0xFD5D_65F4,
    0x0F36_E6F7,
    0x61C6_9362,
    0x93AD_1061,
    0x80FD_E395,
    0x7296_6096,
    0xA65C_047D,
    0x5437_877E,
    0x4767_748A,
    0xB50C_F789,
    0xEB1F_CBAD,
    0x1974_48AE,
    0x0A24_BB5A,
    0xF84F_3859,
    0x2C85_5CB2,
    0xDEEE_DFB1,
    0xCDBE_2C45,
    0x3FD5_AF46,
    0x7198_540D,
    0x83F3_D70E,
    0x90A3_24FA,
    0x62C8_A7F9,
    0xB602_C312,
    0x4469_4011,
    0x5739_B3E5,
    0xA552_30E6,
    0xFB41_0CC2,
    0x092A_8FC1,
    0x1A7A_7C35,
    0xE811_FF36,
    0x3CDB_9BDD,
    0xCEB0_18DE,
    0xDDE0_EB2A,
    0x2F8B_6829,
    0x82F6_3B78,
    0x709D_B87B,
    0x63CD_4B8F,
    0x91A6_C88C,
    0x456C_AC67,
    0xB707_2F64,
    0xA457_DC90,
    0x563C_5F93,
    0x082F_63B7,
    0xFA44_E0B4,
    0xE914_1340,
    0x1B7F_9043,
    0xCFB5_F4A8,
    0x3DDE_77AB,
    0x2E8E_845F,
    0xDCE5_075C,
    0x92A8_FC17,
    0x60C3_7F14,
    0x7393_8CE0,
    0x81F8_0FE3,
    0x5532_6B08,
    0xA759_E80B,
    0xB409_1BFF,
    0x4662_98FC,
    0x1871_A4D8,
    0xEA1A_27DB,
    0xF94A_D42F,
    0x0B21_572C,
    0xDFEB_33C7,
    0x2D80_B0C4,
    0x3ED0_4330,
    0xCCBB_C033,
    0xA24B_B5A6,
    0x5020_36A5,
    0x4370_C551,
    0xB11B_4652,
    0x65D1_22B9,
    0x97BA_A1BA,
    0x84EA_524E,
    0x7681_D14D,
    0x2892_ED69,
    0xDAF9_6E6A,
    0xC9A9_9D9E,
    0x3BC2_1E9D,
    0xEF08_7A76,
    0x1D63_F975,
    0x0E33_0A81,
    0xFC58_8982,
    0xB215_72C9,
    0x407E_F1CA,
    0x532E_023E,
    0xA145_813D,
    0x758F_E5D6,
    0x87E4_66D5,
    0x94B4_9521,
    0x66DF_1622,
    0x38CC_2A06,
    0xCAA7_A905,
    0xD9F7_5AF1,
    0x2B9C_D9F2,
    0xFF56_BD19,
    0x0D3D_3E1A,
    0x1E6D_CDEE,
    0xEC06_4EED,
    0xC38D_26C4,
    0x31E6_A5C7,
    0x22B6_5633,
    0xD0DD_D530,
    0x0417_B1DB,
    0xF67C_32D8,
    0xE52C_C12C,
    0x1747_422F,
    0x4954_7E0B,
    0xBB3F_FD08,
    0xA86F_0EFC,
    0x5A04_8DFF,
    0x8ECE_E914,
    0x7CA5_6A17,
    0x6FF5_99E3,
    0x9D9E_1AE0,
    0xD3D3_E1AB,
    0x21B8_62A8,
    0x32E8_915C,
    0xC083_125F,
    0x1449_76B4,
    0xE622_F5B7,
    0xF572_0643,
    0x0719_8540,
    0x590A_B964,
    0xAB61_3A67,
    0xB831_C993,
    0x4A5A_4A90,
    0x9E90_2E7B,
    0x6CFB_AD78,
    0x7FAB_5E8C,
    0x8DC0_DD8F,
    0xE330_A81A,
    0x115B_2B19,
    0x020B_D8ED,
    0xF060_5BEE,
    0x24AA_3F05,
    0xD6C1_BC06,
    0xC591_4FF2,
    0x37FA_CCF1,
    0x69E9_F0D5,
    0x9B82_73D6,
    0x88D2_8022,
    0x7AB9_0321,
    0xAE73_67CA,
    0x5C18_E4C9,
    0x4F48_173D,
    0xBD23_943E,
    0xF36E_6F75,
    0x0105_EC76,
    0x1255_1F82,
    0xE03E_9C81,
    0x34F4_F86A,
    0xC69F_7B69,
    0xD5CF_889D,
    0x27A4_0B9E,
    0x79B7_37BA,
    0x8BDC_B4B9,
    0x988C_474D,
    0x6AE7_C44E,
    0xBE2D_A0A5,
    0x4C46_23A6,
    0x5F16_D052,
    0xAD7D_5351,
];

fn crc32c_calc(buf: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF;

    for &b in buf {
        crc = CRC32C_LO_TABLE
            [usize::try_from((crc ^ u32::from(b)) & 0xff).expect("invalid chksum byte size")]
            ^ (crc >> 8);
    }

    crc
}
