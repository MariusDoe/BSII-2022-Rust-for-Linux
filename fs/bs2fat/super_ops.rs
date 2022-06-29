use core::ops::{Deref, DerefMut};

use kernel::{
    bindings,
    fs::{inode::Inode, super_block::SuperBlock, super_operations::SuperOperations},
    print::ExpectK,
    sync::Mutex,
};

use crate::{time::SECS_PER_MIN, BS2FatMountOptions, FAT12_MAX_CLUSTERS, FAT16_MAX_CLUSTERS};

pub fn msdos_sb(sb: &mut SuperBlock) -> &mut BS2FatSuperOps {
    // TODO: use own type for this void* field?
    //&*((*sb).s_fs_info as *const T)
    unsafe {
        (sb.s_fs_info as *mut BS2FatSuperOps)
            .as_mut()
            .expectk("msdos_sb in s_fs_info is null!")
    }
}

/// The super operations for BS2FAT
pub struct BS2FatSuperOps {
    pub info: BS2FatSuperInfo,
    pub mutex: BS2FatSuperMutex,
}

impl BS2FatSuperOps {
    /// Constructs a pinned instance.
    ///
    /// # Safety
    ///
    /// The caller must call [`Mutex::init_lock`] on all mutexes before using them.
    pub unsafe fn new_from_info(info: BS2FatSuperInfo) -> Self {
        // SAFETY: guaranteed by caller
        let mutex = unsafe { BS2FatSuperMutex::new_uninit() };
        Self { info, mutex }
    }
}

impl Deref for BS2FatSuperOps {
    type Target = BS2FatSuperInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl DerefMut for BS2FatSuperOps {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

/// Contains various information describing the file system
#[derive(Default)]
pub struct BS2FatSuperInfo {
    pub sectors_per_cluster: u16,
    pub cluster_bits: u16,
    pub cluster_size: u32,

    /// number of tables
    pub fats: u8,
    /// 12, 16 (, 32)
    pub fat_bits: u8,
    pub fat_start: u16,
    pub fat_length: u16,

    pub dir_start: usize,
    pub dir_entries: u16,

    pub data_start: usize,

    /// maximum cluster number
    pub max_cluster: usize,
    pub root_cluster: isize,
    pub previous_free: u32,
    pub free_clusters: u32, /* C sets this to -1 sometimes, we probably want to use u32::MAX for that */
    pub free_clusters_valid: u32,

    pub options: BS2FatMountOptions,

    /// directory entries per block
    pub dir_per_block: i32,
    pub dir_per_block_bits: i32,

    pub volume_id: u32,

    pub fat_inode: Option<*mut Inode>,
    pub fsinfo_inode: Option<*mut Inode>,

    /// fs state before mount
    pub dirty: u32,
}

/// Contains mutexes used for implementing the super operations
pub struct BS2FatSuperMutex {
    // niklas: Mutex around () is closest to the C way
    // if users of the guarded values _always_ lock the mutex, we can move the protected value into
    // the Mutex as one would do in Rust
    pub fat_lock: Mutex<()>,
    // pub nfs_build_inode_lock: Mutex<()>, // we don't need that I think
    pub s_lock: Mutex<()>,
}

impl BS2FatSuperMutex {
    /// Constructs self with uninitialised mutexes.
    ///
    /// # Safety
    ///
    /// The caller must call [`Mutex::init_lock`] on all mutexes before using them.
    unsafe fn new_uninit() -> Self {
        // SAFETY: guaranteed by caller
        unsafe {
            Self {
                fat_lock: Mutex::new(()),
                s_lock: Mutex::new(()),
            }
        }
    }
}

// FIXME there isn't much to say, is there?
unsafe impl Send for BS2FatSuperOps {}
unsafe impl Sync for BS2FatSuperOps {}

impl BS2FatSuperInfo {
    pub fn is_fat16(&self) -> bool {
        self.fat_bits == 16
    }

    pub fn max_fats(&self) -> usize {
        if self.is_fat16() {
            FAT16_MAX_CLUSTERS
        } else {
            FAT12_MAX_CLUSTERS
        }
    }

    pub fn timezone_offset(&self) -> i64 {
        let minutes = if self.options.timezone_set {
            -self.options.time_offset
        } else {
            unsafe { bindings::sys_tz }.tz_minuteswest as _
        };
        minutes * SECS_PER_MIN
    }
}

impl SuperOperations for BS2FatSuperOps {
    kernel::declare_super_operations!();
}
