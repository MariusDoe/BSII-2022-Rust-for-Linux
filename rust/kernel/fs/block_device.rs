//! Since `bindgen` generates `struct block_device` as a ZST, we redefine all the fields here...

use core::mem;

use crate::{bindings, c_types::*};

#[repr(C)]
pub struct BlockDevice {
    pub bd_start_sect: bindings::sector_t,
    pub bd_stats: *mut (), /* bindings::disk_stats, TODO: implement struct disk_stats // has __per_cpu macro in C */
    pub bd_stamp: c_ulong,
    pub bd_read_only: bool, // read-only policy
    pub bd_dev: bindings::dev_t,
    pub bd_openers: c_int,
    pub bd_inode: *mut bindings::inode, // will die
    pub bd_super: *mut bindings::super_block,
    pub bd_mutex: *mut bindings::mutex, // open/close mutex
    pub bd_claiming: *mut c_void,
    pub bd_device: bindings::device,
    pub bd_holder: *mut c_void,
    pub bd_holders: c_int,
    pub bd_write_holder: bool,
    #[cfg(CONFIG_SYSFS)]
    pub bd_holder_disks: bindings::list_head,
    pub bd_holder_dir: *mut bindings::kobject,
    pub bd_partno: u8,
    pub bd_part_count: c_uint, // number of times partitions within this device have been opened
    pub bd_size_lock: bindings::spinlock_t, // for bd_inode->i_size updates
    pub bd_disk: *mut (),      //bindings::gendisk, TODO: implement struct gendisk
    pub bd_bdi: *mut bindings::backing_dev_info,
    pub bd_fsfreeze_count: c_int, // the counter of freeze processes
    pub bd_fsfreeze_mutex: bindings::mutex, // mutex for freeze
    pub bd_fsfreeze_sb: *mut bindings::super_block,
    pub bd_meta_info: *mut (), /* bindings::partition_meta_info, TODO: implement struct partition_meta_info */
    #[cfg(CONFIG_FAIL_MAKE_REQUEST)]
    pub bd_make_it_fail: bool,
}

impl BlockDevice {
    pub fn as_ptr_mut(&mut self) -> *mut bindings::block_device {
        self as *mut _ as *mut _
    }
}

impl AsRef<BlockDevice> for bindings::block_device {
    fn as_ref(&self) -> &BlockDevice {
        unsafe { mem::transmute(self) }
    }
}
impl AsMut<BlockDevice> for bindings::block_device {
    fn as_mut(&mut self) -> &mut BlockDevice {
        unsafe { mem::transmute(self) }
    }
}
