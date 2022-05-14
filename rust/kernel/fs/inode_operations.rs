// SPDX-License-Identifier: GPL-2.0

//! Inode operations.
//!
//! C header: [`include/linux/fs.h`](../../../../include/linux/fs.h)

use core::marker;

use crate::{
    bindings, c_types,
    error::{Error, Result},
    from_kernel_result,
    fs::{dentry::Dentry, inode::Inode, BuildVtable},
    print::ExpectK,
    str::CStr,
    types::{Dev, Iattr, Kstat, Mode, ModeInt, Path, UserNamespace},
};

/// Corresponds to the kernel's `struct inode_operations`.
///
/// You implement this trait whenever you would create a `struct inode_operations`.
///
/// File descriptors may be used from multiple threads/processes concurrently, so your type must be
/// [`Sync`]. It must also be [`Send`] because [`FileOperations::release`] will be called from the
/// thread that decrements that associated file's refcount to zero.
pub trait InodeOperations: Send + Sync + Sized + Default {
    /// The methods to use to populate [`struct inode_operations`].
    const TO_USE: ToUse;

    fn getattr(
        &self,
        _mnt_userns: &mut UserNamespace,
        _path: &Path,
        _stat: &mut Kstat,
        _request_mask: u32,
        _query_flags: u32,
    ) -> Result {
        Err(Error::EINVAL)
    }

    fn setattr(
        &self,
        _mnt_userns: &mut UserNamespace,
        _dentry: &mut Dentry,
        _iattr: &mut Iattr,
    ) -> Result {
        Err(Error::EINVAL)
    }

    fn create(
        &self,
        _mnt_userns: &mut UserNamespace,
        _dir: &mut Inode,
        _dentry: &mut Dentry,
        _mode: Mode,
        _excl: bool,
    ) -> Result {
        Err(Error::EINVAL)
    }
    fn lookup(
        &self,
        _dir: &mut Inode,
        _dentry: &mut Dentry,
        _flags: c_types::c_uint,
    ) -> Result<*mut Dentry> {
        Err(Error::EINVAL)
    }
    fn link(&self, _old_dentry: &mut Dentry, _dir: &mut Inode, _dentry: &mut Dentry) -> Result {
        Err(Error::EINVAL)
    }
    fn unlink(&self, _dir: &mut Inode, _dentry: &mut Dentry) -> Result {
        Err(Error::EINVAL)
    }
    fn symlink(
        &self,
        _mnt_userns: &mut UserNamespace,
        _dir: &mut Inode,
        _dentry: &mut Dentry,
        _symname: &'static CStr,
    ) -> Result {
        Err(Error::EINVAL)
    }
    fn mkdir(
        &self,
        _mnt_userns: &mut UserNamespace,
        _dir: &mut Inode,
        _dentry: &mut Dentry,
        _mode: Mode,
    ) -> Result {
        Err(Error::EINVAL)
    }
    fn rmdir(&self, _dir: &mut Inode, _dentry: &mut Dentry) -> Result {
        Err(Error::EINVAL)
    }
    fn mknod(
        &self,
        _mnt_userns: &mut UserNamespace,
        _dir: &mut Inode,
        _dentry: &mut Dentry,
        _mode: Mode,
        _dev: Dev,
    ) -> Result {
        Err(Error::EINVAL)
    }
    fn rename(
        &self,
        _mnt_userns: &mut UserNamespace,
        _old_dir: &mut Inode,
        _old_dentry: &mut Dentry,
        _new_dir: &mut Inode,
        _new_dentry: &mut Dentry,
        _flags: c_types::c_uint,
    ) -> Result {
        Err(Error::EINVAL)
    }
}

unsafe extern "C" fn setattr_callback<T: InodeOperations>(
    mnt_userns: *mut bindings::user_namespace,
    dentry: *mut bindings::dentry,
    iattr: *mut bindings::iattr,
) -> c_types::c_int {
    unsafe {
        let dentry = dentry.as_mut().expectk("setattr got null dentry").as_mut();
        let inode = dentry.d_inode; // use d_inode method instead?
        let i_ops = &*((*inode).i_private as *const T);
        from_kernel_result! {
            i_ops.setattr(&mut (*mnt_userns), dentry, &mut (*iattr)).map(|()| 0)
        }
    }
}

unsafe extern "C" fn getattr_callback<T: InodeOperations>(
    mnt_userns: *mut bindings::user_namespace,
    path: *const bindings::path,
    stat: *mut bindings::kstat,
    request_mask: u32,
    query_flags: u32,
) -> c_types::c_int {
    unsafe {
        let dentry = (*path).dentry;
        let inode = (*dentry).d_inode; // use d_inode method instead?
        let i_ops = &*((*inode).i_private as *const T);
        from_kernel_result! {
            i_ops.getattr(&mut (*mnt_userns), &(*path), &mut (*stat), request_mask, query_flags).map(|()| 0)
        }
    }
}

unsafe extern "C" fn create_callback<T: InodeOperations>(
    mnt_userns: *mut bindings::user_namespace,
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
    mode: ModeInt,
    excl: bool,
) -> c_types::c_int {
    unsafe {
        let dir = dir.as_mut().expectk("create got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let dentry = dentry.as_mut().expectk("create got null dentry").as_mut();
        from_kernel_result! {
            i_ops.create(&mut (*mnt_userns), dir, dentry, Mode::from_int(mode), excl).map(|()| 0)
        }
    }
}
unsafe extern "C" fn lookup_callback<T: InodeOperations>(
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
    flags: c_types::c_uint,
) -> *mut bindings::dentry {
    unsafe {
        let dir = dir.as_mut().expectk("lookup got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let dentry = dentry.as_mut().expectk("lookup got null dentry").as_mut();
        ret_err_ptr!(i_ops.lookup(dir, dentry, flags).map(|p| p as *mut _))
    }
}
unsafe extern "C" fn link_callback<T: InodeOperations>(
    old_dentry: *mut bindings::dentry,
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
) -> c_types::c_int {
    unsafe {
        let dir = dir.as_mut().expectk("link got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let old_dentry = old_dentry
            .as_mut()
            .expectk("link got null old_dentry")
            .as_mut();
        let dentry = dentry.as_mut().expectk("link got null dentry").as_mut();
        from_kernel_result! {
            i_ops.link(old_dentry, dir, dentry).map(|()| 0)
        }
    }
}
unsafe extern "C" fn unlink_callback<T: InodeOperations>(
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
) -> c_types::c_int {
    unsafe {
        let dir = dir.as_mut().expectk("unlink got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let dentry = dentry.as_mut().expectk("unlink got null dentry").as_mut();
        from_kernel_result! {
            i_ops.unlink(dir, dentry).map(|()| 0)
        }
    }
}
unsafe extern "C" fn symlink_callback<T: InodeOperations>(
    mnt_userns: *mut bindings::user_namespace,
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
    symname: *const c_types::c_char,
) -> c_types::c_int {
    unsafe {
        let dir = dir.as_mut().expectk("symlink got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let dentry = dentry.as_mut().expectk("symlink got null dentry").as_mut();
        from_kernel_result! {
            i_ops.symlink(&mut (*mnt_userns), dir, dentry, CStr::from_char_ptr(symname)).map(|()| 0)
        }
    }
}
unsafe extern "C" fn mkdir_callback<T: InodeOperations>(
    mnt_userns: *mut bindings::user_namespace,
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
    mode: ModeInt,
) -> c_types::c_int {
    unsafe {
        let dir = dir.as_mut().expectk("mkdir got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let dentry = dentry.as_mut().expectk("mkdir got null dentry").as_mut();
        from_kernel_result! {
            i_ops.mkdir(&mut (*mnt_userns), dir, dentry, Mode::from_int(mode)).map(|()| 0) // todo: mode_t is u32 but u16 in Mode?
        }
    }
}
unsafe extern "C" fn rmdir_callback<T: InodeOperations>(
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
) -> c_types::c_int {
    unsafe {
        let dir = dir.as_mut().expectk("rmdir got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let dentry = dentry.as_mut().expectk("rmdir got null dentry").as_mut();
        from_kernel_result! {
            i_ops.rmdir(dir, dentry).map(|()| 0)
        }
    }
}
unsafe extern "C" fn mknod_callback<T: InodeOperations>(
    mnt_userns: *mut bindings::user_namespace,
    dir: *mut bindings::inode,
    dentry: *mut bindings::dentry,
    mode: ModeInt,
    dev: bindings::dev_t,
) -> c_types::c_int {
    unsafe {
        let dir = dir.as_mut().expectk("mknod got null dir").as_mut();
        let i_ops = &*(dir.i_private as *const T);
        let dentry = dentry.as_mut().expectk("mknod got null dentry").as_mut();
        from_kernel_result! {
            i_ops.mknod(&mut (*mnt_userns), dir, dentry, Mode::from_int(mode), dev).map(|()| 0)
        }
    }
}
unsafe extern "C" fn rename_callback<T: InodeOperations>(
    mnt_userns: *mut bindings::user_namespace,
    old_dir: *mut bindings::inode,
    old_dentry: *mut bindings::dentry,
    new_dir: *mut bindings::inode,
    new_dentry: *mut bindings::dentry,
    flags: c_types::c_uint,
) -> c_types::c_int {
    unsafe {
        let old_dir = old_dir.as_mut().expectk("rename got null dir").as_mut();
        let i_ops = &*(old_dir.i_private as *const T);
        let old_dentry = old_dentry
            .as_mut()
            .expectk("rename got null dentry")
            .as_mut();
        let new_dir = new_dir.as_mut().expectk("rename got null dir").as_mut();
        let new_dentry = new_dentry
            .as_mut()
            .expectk("rename got null dentry")
            .as_mut();
        from_kernel_result! {
            i_ops.rename(&mut (*mnt_userns), old_dir, old_dentry, new_dir, new_dentry, flags).map(|()| 0)
        }
    }
}

pub(crate) struct InodeOperationsVtable<T>(marker::PhantomData<T>);

impl<T: InodeOperations> InodeOperationsVtable<T> {
    const VTABLE: bindings::inode_operations = bindings::inode_operations {
        getattr: if T::TO_USE.getattr {
            Some(getattr_callback::<T>)
        } else {
            None
        },
        setattr: if T::TO_USE.setattr {
            Some(setattr_callback::<T>)
        } else {
            None
        },
        lookup: if T::TO_USE.lookup {
            Some(lookup_callback::<T>)
        } else {
            None
        },
        get_link: None,
        permission: None,
        get_acl: None,
        readlink: None,
        create: if T::TO_USE.create {
            Some(create_callback::<T>)
        } else {
            None
        },
        link: if T::TO_USE.link {
            Some(link_callback::<T>)
        } else {
            None
        },
        unlink: if T::TO_USE.unlink {
            Some(unlink_callback::<T>)
        } else {
            None
        },
        symlink: if T::TO_USE.symlink {
            Some(symlink_callback::<T>)
        } else {
            None
        },
        mkdir: if T::TO_USE.mkdir {
            Some(mkdir_callback::<T>)
        } else {
            None
        },
        rmdir: if T::TO_USE.rmdir {
            Some(rmdir_callback::<T>)
        } else {
            None
        },
        mknod: if T::TO_USE.mknod {
            Some(mknod_callback::<T>)
        } else {
            None
        },
        rename: if T::TO_USE.rename {
            Some(rename_callback::<T>)
        } else {
            None
        },
        listxattr: None,
        fiemap: None,
        update_time: None,
        atomic_open: None,
        tmpfile: None,
        set_acl: None,
        fileattr_get: None,
        fileattr_set: None,
    };
}

impl<T: InodeOperations> BuildVtable<bindings::inode_operations> for InodeOperationsVtable<T> {
    fn build_vtable() -> &'static bindings::inode_operations {
        &Self::VTABLE
    }
}

impl<T: InodeOperations> BuildVtable<bindings::inode_operations> for T {
    fn build_vtable() -> &'static bindings::inode_operations {
        InodeOperationsVtable::<T>::build_vtable()
    }
}

/// Represents which fields of [`struct inode_block_operations`] should be populated with pointers.
pub struct ToUse {
    /// The `lookup` field of [`struct inode_operations`].
    pub lookup: bool,

    /// The `get_link` field of [`struct inode_operations`].
    pub get_link: bool,

    /// The `permission` field of [`struct inode_operations`].
    pub permission: bool,

    /// The `get_acl` field of [`struct inode_operations`].
    pub get_acl: bool,

    /// The `readlink` field of [`struct inode_operations`].
    pub readlink: bool,

    /// The `create` field of [`struct inode_operations`].
    pub create: bool,

    /// The `link` field of [`struct inode_operations`].
    pub link: bool,

    /// The `unlink` field of [`struct inode_operations`].
    pub unlink: bool,

    /// The `symlink` field of [`struct inode_operations`].
    pub symlink: bool,

    /// The `mkdir` field of [`struct inode_operations`].
    pub mkdir: bool,

    /// The `rmdir` field of [`struct inode_operations`].
    pub rmdir: bool,

    /// The `mknod` field of [`struct inode_operations`].
    pub mknod: bool,

    /// The `rename` field of [`struct inode_operations`].
    pub rename: bool,

    /// The `listxattr` field of [`struct inode_operations`].
    pub listxattr: bool,

    /// The `fiemap` field of [`struct inode_operations`].
    pub fiemap: bool,

    /// The `update_time` field of [`struct inode_operations`].
    pub update_time: bool,

    /// The `atomic_open` field of [`struct inode_operations`].
    pub atomic_open: bool,

    /// The `tmpfile` field of [`struct inode_operations`].
    pub tmpfile: bool,

    /// The `set_acl` field of [`struct inode_operations`].
    pub set_acl: bool,

    /// The `getattr` field of [`struct inode_operations`].
    pub getattr: bool,

    /// The `setattr` field of [`struct inode_operations`].
    pub setattr: bool,
}

/// A constant version where all values are to set to `false`, that is, all supported fields will
/// be set to null pointers.
pub const USE_NONE: ToUse = ToUse {
    lookup: false,
    get_link: false,
    permission: false,
    get_acl: false,
    readlink: false,
    create: false,
    link: false,
    unlink: false,
    symlink: false,
    mkdir: false,
    rmdir: false,
    mknod: false,
    rename: false,
    listxattr: false,
    fiemap: false,
    update_time: false,
    atomic_open: false,
    tmpfile: false,
    set_acl: false,
    getattr: false,
    setattr: false,
};

#[macro_export]
macro_rules! declare_inode_operations {
    () => {
        const TO_USE: $crate::fs::inode_operations::ToUse = $crate::fs::inode_operations::USE_NONE;
    };
    ($($i:ident),+) => {
        const TO_USE: kernel::fs::inode_operations::ToUse =
            $crate::fs::inode_operations::ToUse {
                $($i: true),+ ,
                ..$crate::fs::inode_operations::USE_NONE
            };
    };
}
