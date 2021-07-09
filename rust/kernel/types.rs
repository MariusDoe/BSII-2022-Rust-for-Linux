// SPDX-License-Identifier: GPL-2.0

//! Kernel types.
//!
//! C header: [`include/linux/types.h`](../../../../include/linux/types.h)

use core::{
    ops::{BitAnd, BitOr, Deref},
    pin::Pin,
};

use alloc::{boxed::Box, sync::Arc};

use crate::bindings;
use crate::c_types;
use crate::declare_constant_from_bindings;
use crate::sync::{Ref, RefCounted};

pub type UserNamespace = bindings::user_namespace;
pub type Iattr = bindings::iattr;
pub type Path = bindings::path;
pub type Kstat = bindings::kstat;
pub type Dev = bindings::dev_t;
pub type Page = bindings::page;

macro_rules! impl_flag_methods {
    ($T:ty, $V:ty) => {
        impl $T {
            pub const fn from_int(val: $V) -> Self {
                Self(val)
            }
            pub const fn into_int(self) -> $V {
                self.0
            }
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }
            pub const fn has(self, other: Self) -> bool {
                self.0 & other.0 != 0
            }
            pub const fn with(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
            pub const fn without(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }
        }
    };
}

pub struct FileSystemFlags(c_types::c_int);

#[rustfmt::skip]
impl FileSystemFlags {
    /// Not a virtual file system. An actual underlying block device is required.
    pub const FS_REQUIRES_DEV: Self         = Self::from_int(bindings::FS_REQUIRES_DEV as _);

    /// Mount data is binary, and cannot be handled by the standard option parser
    pub const FS_BINARY_MOUNTDATA: Self     = Self::from_int(bindings::FS_BINARY_MOUNTDATA as _);

    /// Has subtype
    pub const FS_HAS_SUBTYPE: Self          = Self::from_int(bindings::FS_HAS_SUBTYPE as _);

    /// Can be mounted by userns root
    pub const FS_USERNS_MOUNT: Self         = Self::from_int(bindings::FS_USERNS_MOUNT as _);

    /// Disable fanotify permission events
    pub const FS_DISALLOW_NOTIFY_PERM: Self = Self::from_int(bindings::FS_DISALLOW_NOTIFY_PERM as _);

    /// FS has been updated to handle vfs idmappings
    pub const FS_ALLOW_IDMAP: Self          = Self::from_int(bindings::FS_ALLOW_IDMAP as _);

    /// Remove once all fs converted
    pub const FS_THP_SUPPORT: Self          = Self::from_int(bindings::FS_THP_SUPPORT as _);

    /// FS will handle d_move() during rename() internally
    pub const FS_RENAME_DOES_D_MOVE: Self   = Self::from_int(bindings::FS_RENAME_DOES_D_MOVE as _);
}

impl_flag_methods!(FileSystemFlags, c_types::c_int);

/// Permissions.
///
/// C header: [`include/uapi/linux/stat.h`](../../../../include/uapi/linux/stat.h)
///
/// C header: [`include/linux/stat.h`](../../../../include/linux/stat.h)
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Mode(bindings::umode_t);
pub type ModeInt = u16;

impl Mode {
    /// Creates a [`Mode`] from an integer.
    pub const fn from_int(m: ModeInt) -> Mode {
        Mode(m)
    }

    /// Returns the mode as an integer.
    pub fn as_int(&self) -> ModeInt {
        self.0
    }
}

#[rustfmt::skip]
impl Mode {
    // See `man 7 inode`.

    // file type
    declare_constant_from_bindings!(S_IFMT, "bit mask for the file type bit field");

    declare_constant_from_bindings!(S_IFSOCK, "socket");
    declare_constant_from_bindings!(S_IFLNK,  "symbolic link");
    declare_constant_from_bindings!(S_IFREG,  "regular file");
    declare_constant_from_bindings!(S_IFBLK,  "block device");
    declare_constant_from_bindings!(S_IFDIR,  "directory");
    declare_constant_from_bindings!(S_IFCHR,  "character device");
    declare_constant_from_bindings!(S_IFIFO,  "FIFO");

    // file mode component of the st_mode field
    declare_constant_from_bindings!(S_ISUID,  "set-user-ID bit (see execve(2))");
    declare_constant_from_bindings!(S_ISGID,  "set-group-ID bit (see below)");
    declare_constant_from_bindings!(S_ISVTX,  "sticky bit (see below)");

    declare_constant_from_bindings!(S_IRWXU,  "owner has read, write, and execute permission");
    declare_constant_from_bindings!(S_IRUSR,  "owner has read permission");
    declare_constant_from_bindings!(S_IWUSR,  "owner has write permission");
    declare_constant_from_bindings!(S_IXUSR,  "owner has execute permission");

    declare_constant_from_bindings!(S_IRWXG,  "group has read, write, and execute permission");
    declare_constant_from_bindings!(S_IRGRP,  "group has read permission");
    declare_constant_from_bindings!(S_IWGRP,  "group has write permission");
    declare_constant_from_bindings!(S_IXGRP,  "group has execute permission");

    declare_constant_from_bindings!(S_IRWXO,  "others (not in group) have read, write, and execute permission");
    declare_constant_from_bindings!(S_IROTH,  "others have read permission");
    declare_constant_from_bindings!(S_IWOTH,  "others have write permission");
    declare_constant_from_bindings!(S_IXOTH,  "others have execute permission");

    // extras
    declare_constant_from_bindings!(S_IRWXUGO, "");
}

impl BitAnd for Mode {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for Mode {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Used to convert an object into a raw pointer that represents it.
///
/// It can eventually be converted back into the object. This is used to store objects as pointers
/// in kernel data structures, for example, an implementation of [`FileOperations`] in `struct
/// file::private_data`.
pub trait PointerWrapper {
    /// Returns the raw pointer.
    fn into_pointer(self) -> *const c_types::c_void;

    /// Returns the instance back from the raw pointer.
    ///
    /// # Safety
    ///
    /// The passed pointer must come from a previous call to [`PointerWrapper::into_pointer()`].
    unsafe fn from_pointer(ptr: *const c_types::c_void) -> Self;
}

impl<T> PointerWrapper for Box<T> {
    fn into_pointer(self) -> *const c_types::c_void {
        Box::into_raw(self) as _
    }

    unsafe fn from_pointer(ptr: *const c_types::c_void) -> Self {
        // SAFETY: The passed pointer comes from a previous call to [`Self::into_pointer()`].
        unsafe { Box::from_raw(ptr as _) }
    }
}

impl<T: RefCounted> PointerWrapper for Ref<T> {
    fn into_pointer(self) -> *const c_types::c_void {
        Ref::into_raw(self) as _
    }

    unsafe fn from_pointer(ptr: *const c_types::c_void) -> Self {
        // SAFETY: The passed pointer comes from a previous call to [`Self::into_pointer()`].
        unsafe { Ref::from_raw(ptr as _) }
    }
}

impl<T> PointerWrapper for Arc<T> {
    fn into_pointer(self) -> *const c_types::c_void {
        Arc::into_raw(self) as _
    }

    unsafe fn from_pointer(ptr: *const c_types::c_void) -> Self {
        // SAFETY: The passed pointer comes from a previous call to [`Self::into_pointer()`].
        unsafe { Arc::from_raw(ptr as _) }
    }
}

impl<T: PointerWrapper + Deref> PointerWrapper for Pin<T> {
    fn into_pointer(self) -> *const c_types::c_void {
        // SAFETY: We continue to treat the pointer as pinned by returning just a pointer to it to
        // the caller.
        let inner = unsafe { Pin::into_inner_unchecked(self) };
        inner.into_pointer()
    }

    unsafe fn from_pointer(p: *const c_types::c_void) -> Self {
        // SAFETY: The object was originally pinned.
        // The passed pointer comes from a previous call to `inner::into_pointer()`.
        unsafe { Pin::new_unchecked(T::from_pointer(p)) }
    }
}

/// Runs a cleanup function/closure when dropped.
///
/// The [`ScopeGuard::dismiss`] function prevents the cleanup function from running.
///
/// # Examples
///
/// In the example below, we have multiple exit paths and we want to log regardless of which one is
/// taken:
/// ```
/// # use kernel::prelude::*;
/// # use kernel::ScopeGuard;
/// fn example1(arg: bool) {
///     let _log = ScopeGuard::new(|| pr_info!("example1 completed\n"));
///
///     if arg {
///         return;
///     }
///
///     // Do something...
/// }
/// ```
///
/// In the example below, we want to log the same message on all early exits but a different one on
/// the main exit path:
/// ```
/// # use kernel::prelude::*;
/// # use kernel::ScopeGuard;
/// fn example2(arg: bool) {
///     let log = ScopeGuard::new(|| pr_info!("example2 returned early\n"));
///
///     if arg {
///         return;
///     }
///
///     // (Other early returns...)
///
///     log.dismiss();
///     pr_info!("example2 no early return\n");
/// }
/// ```
pub struct ScopeGuard<T: FnOnce()> {
    cleanup_func: Option<T>,
}

impl<T: FnOnce()> ScopeGuard<T> {
    /// Creates a new cleanup object with the given cleanup function.
    pub fn new(cleanup_func: T) -> Self {
        Self {
            cleanup_func: Some(cleanup_func),
        }
    }

    /// Prevents the cleanup function from running.
    pub fn dismiss(mut self) {
        self.cleanup_func.take();
    }
}

impl<T: FnOnce()> Drop for ScopeGuard<T> {
    fn drop(&mut self) {
        // Run the cleanup function if one is still present.
        if let Some(cleanup) = self.cleanup_func.take() {
            cleanup();
        }
    }
}
