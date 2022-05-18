// SPDX-License-Identifier: GPL-2.0

//! Super operations.
//!
//! C header: [`include/linux/fs.h`](../../../../include/linux/fs.h)

use core::marker;

use crate::{
    bindings, c_types,
    error::{from_kernel_result, Error, Result},
    fs::dentry::Dentry,
    fs::inode::Inode,
    print::ExpectK,
};

pub type SeqFile = bindings::seq_file;
pub type Kstatfs = bindings::kstatfs;

// unsafe extern "C" fn alloc_inode_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
// ) -> *mut bindings::inode {
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     let inode = s_ops.alloc_inode(&SuperBlock::from_ptr(sb)); // TODO SuperBlock, Inode
//     inode.map(|i| Inode::into_ptr(inode))
// }

// unsafe extern "C" fn destroy_inode_callback<T: SuperOperations>(
//     inode: *mut bindings::inode,
// ) {
//     let sb = (*inode).i_sb as *const bindings::super_block;
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     let inode = Inode::from_ptr(inode);
//     s_ops.destroy_inode(inode);
// }

// unsafe extern "C" fn free_inode_callback<T: SuperOperations>(
//     inode: *mut bindings::inode,
// ) {
//     let sb = (*inode).i_sb as *const bindings::super_block;
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     let inode = Inode::from_ptr(inode);
//     s_ops.free_inode(inode);
// }

// unsafe extern "C" fn dirty_inode_callback<T: SuperOperations>(
//     inode: *mut bindings::inode,
//     flags: c_types::c_int,
// ) {
//     let sb = (*inode).i_sb as *const bindings::super_block;
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     let inode = Inode::from_ptr(inode);
//     s_ops.dirty_inode(inode, flags);
// }

// unsafe extern "C" fn write_inode_callback<T: SuperOperations>(
//     inode: *mut bindings::inode,
//     wbc: *mut bindings::writeback_control, // TODO
// ) -> c_types::c_int {
//     let sb = (*inode).i_sb as *const bindings::super_block;
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     let inode = Inode::from_ptr(inode);
//     let wbc = WritebackControl::from_ptr(wbc);
//     from_kernel_result! {
//         s_ops.write_inode(inode, wbc)?;
//         Ok(0)
//     }
// }

unsafe extern "C" fn drop_inode_callback<T: SuperOperations>(
    inode: *mut bindings::inode,
) -> c_types::c_int {
    unsafe {
        let sb = (*inode).i_sb as *const bindings::super_block;
        let s_ops = &*((*sb).s_fs_info as *const T);
        let inode = inode.as_mut().expectk("drop_inode got null inode").as_mut();
        from_kernel_result! {
            s_ops.drop_inode(inode)?;
            Ok(0)
        }
    }
}

// unsafe extern "C" fn evict_inode_callback<T: SuperOperations>(
//     inode: *mut bindings::inode,
// ) {
//     let sb = (*inode).i_sb as *const bindings::super_block;
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     let inode = Inode::from_ptr(inode);
//     s_ops.evict_inode(inode);
// }

// unsafe extern "C" fn put_super_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
// ) {
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     s_ops.put_super(&SuperBlock::from_ptr(sb));
// }

// unsafe extern "C" fn sync_fs_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
//     wait: c_types::c_int,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.sync_fs(&SuperBlock::from_ptr(sb), wait)?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn freeze_super_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.sync_fs(&SuperBlock::from_ptr(sb))?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn freeze_fs_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.freeze_fs(&SuperBlock::from_ptr(sb))?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn thaw_super_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.thaw_super(&SuperBlock::from_ptr(sb))?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn unfreeze_fs_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.unfreeze_fs(&SuperBlock::from_ptr(sb))?;
//         Ok(0)
//     }
// }

unsafe extern "C" fn statfs_callback<T: SuperOperations>(
    root: *mut bindings::dentry,
    buf: *mut bindings::kstatfs,
) -> c_types::c_int {
    from_kernel_result! {
        unsafe {
            let sb = (*root).d_sb as *const bindings::super_block;
            let s_ops = &*((*sb).s_fs_info as *const T);
            s_ops.statfs(root.as_mut().expectk("Statfs got null dentry").as_mut(), &mut *buf)?;
            Ok(0)
        }
    }
}

// unsafe extern "C" fn remount_fs_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
//     flags: *mut c_types::c_int,
//     data: *mut c_types::c_char,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.remount_fs(
//             &SuperBlock::from_ptr(sb),
//             flags,
//             &CStr::from_ptr(data), // TODO
//         )?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn umount_begin_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
// ) {
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     s_ops.umount_begin(&SuperBlock::from_ptr(sb));
// }

unsafe extern "C" fn show_options_callback<T: SuperOperations>(
    s: *mut bindings::seq_file,
    root: *mut bindings::dentry,
) -> c_types::c_int {
    from_kernel_result! {
        unsafe {
            let sb = (*root).d_sb as *const bindings::super_block;
            let s_ops = &*((*sb).s_fs_info as *const T);
            s_ops.show_options(&mut *s, root.as_mut().expectk("show_options got null dentry").as_mut())?;
            Ok(0)
        }
    }
}

// unsafe extern "C" fn show_devname_callback<T: SuperOperations>(
//     s: *mut bindings::seq_file,
//     root: *mut bindings::dentry,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let sb = (*root).d_sb as *const bindings::super_block;
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.show_devname(&SeqFile::from_ptr(s), &Dentry::from_ptr(root))?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn show_path_callback<T: SuperOperations>(
//     s: *mut bindings::seq_file,
//     root: *mut bindings::dentry,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let sb = (*root).d_sb as *const bindings::super_block;
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.show_path(&SeqFile::from_ptr(s), &Dentry::from_ptr(root))?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn show_stats_callback<T: SuperOperations>(
//     s: *mut bindings::seq_file,
//     root: *mut bindings::dentry,
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let sb = (*root).d_sb as *const bindings::super_block;
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.show_stats(&SeqFile::from_ptr(s), &Dentry::from_ptr(root))?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn bdev_try_to_free_page_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
//     page: *mut bindings::page, // TODO
//     wait: bindings::gfp_t, // TODO
// ) -> c_types::c_int {
//     from_kernel_result! {
//         let s_ops = &*((*sb).s_fs_info as *const T);
//         s_ops.show_stats(&SuperBlock::from_ptr(sb), &Page::from_ptr(page), wait)?;
//         Ok(0)
//     }
// }

// unsafe extern "C" fn nr_cached_objects_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
//     sc: *mut bindings::shrink_control, // TODO
// ) -> c_types::c_long {
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     s_ops.nr_cached_objects(&SuperBlock::from_ptr(sb), &ShrinkControl::from_ptr(sc))
// }

// unsafe extern "C" fn free_cached_objects_callback<T: SuperOperations>(
//     sb: *mut bindings::super_block,
//     sc: *mut bindings::shrink_control, // TODO
// ) -> c_types::c_long {
//     let s_ops = &*((*sb).s_fs_info as *const T);
//     s_ops.free_cached_objects(&SuperBlock::from_ptr(sb), &ShrinkControl::from_ptr(sc))
// }

pub(crate) struct SuperOperationsVtable<T>(marker::PhantomData<T>);

impl<T: SuperOperations> SuperOperationsVtable<T> {
    const VTABLE: bindings::super_operations = bindings::super_operations {
        // alloc_inode: T::TO_USE.alloc_inode {
        //     Some(alloc_inode_callback::<T>)
        // } else {
        //     None
        // },
        // destroy_inode: if T::TO_USE.destroy_inode {
        //     Some(destroy_inode_callback::<T>)
        // } else {
        //     None
        // },
        // free_inode: if T::TO_USE.free_inode {
        //     Some(free_inode_callback::<T>)
        // } else {
        //     None
        // },
        // dirty_inode: if T::TO_USE.dirty_inode {
        //     Some(dirty_inode_callback::<T>)
        // } else {
        //     None
        // },
        // write_inode: if T::TO_USE.write_inode {
        //     Some(write_inode_callback::<T>)
        // } else {
        //     None
        // },
        drop_inode: if T::TO_USE.drop_inode {
            Some(drop_inode_callback::<T>)
        } else {
            None
        },
        // evict_inode: if T::TO_USE.evict_inode {
        //     Some(evict_inode_callback::<T>)
        // } else {
        //     None
        // },
        // put_super: if T::TO_USE.put_super {
        //     Some(put_super_callback::<T>)
        // } else {
        //     None
        // },
        // sync_fs: if T::TO_USE.sync_fs {
        //     Some(sync_fs_callback::<T>)
        // } else {
        //     None
        // },
        // freeze_super: if T::TO_USE.freeze_super {
        //     Some(freeze_super_callback::<T>)
        // } else {
        //     None
        // },
        // freeze_fs: if T::TO_USE.freeze_fs {
        //     Some(freeze_fs_callback::<T>)
        // } else {
        //     None
        // },
        // thaw_super: if T::TO_USE.thaw_super {
        //     Some(thaw_super_callback::<T>)
        // } else {
        //     None
        // },
        // unfreeze_fs: if T::TO_USE.unfreeze_fs {
        //     Some(unfreeze_fs_callback::<T>)
        // } else {
        //     None
        // },
        statfs: if T::TO_USE.statfs {
            Some(statfs_callback::<T>)
        } else {
            None
        },
        // remount_fs: if T::TO_USE.remount_fs {
        //     Some(remount_fs_callback::<T>)
        // } else {
        //     None
        // },
        // umount_begin: if T::TO_USE.umount_begin {
        //     Some(umount_begin_callback::<T>)
        // } else {
        //     None
        // },
        show_options: if T::TO_USE.show_options {
            Some(show_options_callback::<T>)
        } else {
            None
        },
        // show_devname: if T::TO_USE.show_devname {
        //     Some(show_devname_callback::<T>)
        // } else {
        //     None
        // },
        // show_path: if T::TO_USE.show_path {
        //     Some(show_path_callback::<T>)
        // } else {
        //     None
        // },
        // show_stats: if T::TO_USE.show_stats {
        //     Some(show_stats_callback::<T>)
        // } else {
        //     None
        // },
        // bdev_try_to_free_page: if T::TO_USE.bdev_try_to_free_page {
        //     Some(bdev_try_to_free_page_callback::<T>)
        // } else {
        //     None
        // },
        // nr_cached_objects: if T::TO_USE.nr_cached_objects {
        //     Some(nr_cached_objects_callback::<T>)
        // } else {
        //     None
        // },
        // free_cached_objects: if T::TO_USE.free_cached_objects {
        //     Some(free_cached_objects_callback::<T>)
        // } else {
        //     None
        // },
        alloc_inode: None,
        destroy_inode: None,
        free_inode: None,
        dirty_inode: None,
        write_inode: None,
        evict_inode: None,
        put_super: None,
        sync_fs: None,
        freeze_super: None,
        freeze_fs: None,
        thaw_super: None,
        unfreeze_fs: None,
        remount_fs: None,
        umount_begin: None,
        show_devname: None,
        show_path: None,
        show_stats: None,
        bdev_try_to_free_page: None,
        nr_cached_objects: None,
        free_cached_objects: None,
        get_dquots: None,
        quota_read: None,
        quota_write: None,
    };

    /// Builds an instance of [`struct super_operations`].
    ///
    /// # Safety
    ///
    /// The caller must ensure that the adapter is compatible with the way the device is registered.
    pub(crate) const unsafe fn build() -> &'static bindings::super_operations {
        &Self::VTABLE
    }
}

/// Represents which fields of [`struct super_block_operations`] should be populated with pointers.
pub struct ToUse {
    /// The `alloc_inode` field of [`struct super_operations`].
    pub alloc_inode: bool,

    /// The `destroy_inode` field of [`struct super_operations`].
    pub destroy_inode: bool,

    /// The `free_inode` field of [`struct super_operations`].
    pub free_inode: bool,

    /// The `dirty_inode` field of [`struct super_operations`].
    pub dirty_inode: bool,

    /// The `write_inode` field of [`struct super_operations`].
    pub write_inode: bool,

    /// The `drop_inode` field of [`struct super_operations`].
    pub drop_inode: bool,

    /// The `evict_inode` field of [`struct super_operations`].
    pub evict_inode: bool,

    /// The `put_super` field of [`struct super_operations`].
    pub put_super: bool,

    /// The `sync_fs` field of [`struct super_operations`].
    pub sync_fs: bool,

    /// The `freeze_super` field of [`struct super_operations`].
    pub freeze_super: bool,

    /// The `freeze_fs` field of [`struct super_operations`].
    pub freeze_fs: bool,

    /// The `thaw_super` field of [`struct super_operations`].
    pub thaw_super: bool,

    /// The `unfreeze_fs` field of [`struct super_operations`].
    pub unfreeze_fs: bool,

    /// The `statfs` field of [`struct super_operations`].
    pub statfs: bool,

    /// The `remount_fs` field of [`struct super_operations`].
    pub remount_fs: bool,

    /// The `umount_begin` field of [`struct super_operations`].
    pub umount_begin: bool,

    /// The `show_options` field of [`struct super_operations`].
    pub show_options: bool,

    /// The `show_devname` field of [`struct super_operations`].
    pub show_devname: bool,

    /// the `show_path` field of [`struct super_operations`].
    pub show_path: bool,

    /// the `show_stats` field of [`struct super_operations`].
    pub show_stats: bool,

    /// the `bdev_try_to_free_page` field of [`struct super_operations`].
    pub bdev_try_to_free_page: bool,

    /// the `nr_cached_objects` field of [`struct super_operations`].
    pub nr_cached_objects: bool,

    /// the `free_cached_objects` field of [`struct super_operations`].
    pub free_cached_objects: bool,
}

/// A constant version where all values are to set to `false`, that is, all supported fields will
/// be set to null pointers.
pub const USE_NONE: ToUse = ToUse {
    alloc_inode: false,
    destroy_inode: false,
    free_inode: false,
    dirty_inode: false,
    write_inode: false,
    drop_inode: false,
    evict_inode: false,
    put_super: false,
    sync_fs: false,
    freeze_super: false,
    freeze_fs: false,
    thaw_super: false,
    unfreeze_fs: false,
    statfs: false,
    remount_fs: false,
    umount_begin: false,
    show_options: false,
    show_devname: false,
    show_path: false,
    show_stats: false,
    bdev_try_to_free_page: false,
    nr_cached_objects: false,
    free_cached_objects: false,
};

#[macro_export]
macro_rules! declare_super_operations {
    () => {
        const TO_USE: $crate::fs::super_operations::ToUse = $crate::fs::super_operations::USE_NONE;
    };
    ($($i:ident),+) => {
        const TO_USE: kernel::fs::super_operations::ToUse =
            $crate::fs::super_operations::ToUse {
                $($i: true),+ ,
                ..$crate::fs::super_operations::USE_NONE
            };
    };
}

/// Corresponds to the kernel's `struct super_operations`.
///
/// You implement this trait whenever you would create a `struct super_operations`.
///
/// File descriptors may be used from multiple threads/processes concurrently, so your type must be
/// [`Sync`]. It must also be [`Send`] because [`FileOperations::release`] will be called from the
/// thread that decrements that associated file's refcount to zero.
pub trait SuperOperations: Send + Sync + Sized + Default {
    /// The methods to use to populate [`struct super_operations`].
    const TO_USE: ToUse;

    // fn alloc_inode(&self, _sb: &SuperBlock) -> Option<Inode> {
    //     None
    // }

    // fn destroy_inode(&self, _inode: &Inode) {}

    // fn free_inode(&self, _inode: &Inode) {}

    // fn dirty_inode(&self, _inode: &Inode, _flags: i32) {}

    // fn write_inode(&self, _inode: &Inode, _wbc: &WritebackControl) -> Result {
    //     Err(Error::EINVAL)
    // }

    fn drop_inode(&self, _inode: &mut Inode) -> Result {
        Err(Error::EINVAL)
    }

    // fn evict_inode(&self, _inode: &Inode) {}

    // fn put_super(&self, _sb: &SuperBlock) {}

    // fn sync_fs(&self, _sb: &SuperBlock, wait: i32) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn freeze_super(&self, _sb: &SuperBlock) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn freeze_fs(&self, _sb: &SuperBlock) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn thaw_super(&self, _sb: &SuperBlock) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn unfreeze_fs(&self, _sb: &SuperBlock) -> Result {
    //     Err(Error::EINVAL)
    // }

    fn statfs(&self, _root: &mut Dentry, _buf: &mut Kstatfs) -> Result {
        Err(Error::EINVAL)
    }

    // fn remount_fs(&self, _sb: &SuperBlock, _flags: i32, _data: &CStr) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn umount_begin(&self, _sb: &SuperBlock) {}

    fn show_options(&self, _s: &mut SeqFile, _root: &mut Dentry) -> Result {
        Err(Error::EINVAL)
    }

    // fn show_devname(&self, _s: &SeqFile, _root: &Dentry) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn show_path(&self, _s: &SeqFile, _root: &Dentry) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn show_stats(&self, _s: &SeqFile, _root: &Dentry) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn bdev_try_to_free_page(&self, _sb: &SuperBlock, _page: &Page, _wait: GfpT) -> Result {
    //     Err(Error::EINVAL)
    // }

    // fn nr_cached_objects(&self, _sb: &SuperBlock, _sc: &ShrinkControl) -> i64 {
    //     0
    // }

    // fn free_cached_objects(&self, _sb: &SuperBlock, _sc: &ShrinkControl) -> i64 {
    //     0
    // }
}
