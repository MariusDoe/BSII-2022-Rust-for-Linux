// SPDX-License-Identifier: GPL-2.0

//! AddressSpace operations.
//!
//! C header: [`include/linux/fs.h`](../../../../include/linux/fs.h)

use core::marker;

use crate::{
    bindings, c_types,
    error::{from_kernel_result, Error, Result},
    file::File,
    fs::BuildVtable,
    types::{AddressSpace, Page, Folio},
};

/// Corresponds to the kernel's `struct adress_space_operations`.
///
/// You implement this trait whenever you would create a `struct adress_space_operations`.
///
/// File descriptors may be used from multiple threads/processes concurrently, so your type must be
/// [`Sync`]. It must also be [`Send`] because [`FileOperations::release`] will be called from the
/// thread that decrements that associated file's refcount to zero.
pub trait AddressSpaceOperations: Send + Sync + Sized + Default {
    /// The methods to use to populate [`struct adress_space_operations`].
    const TO_USE: ToUse;

    fn readpage(&self, _file: &File, _page: &mut Page) -> Result {
        Err(Error::EINVAL)
    }

    fn write_begin(
        &self,
        _file: Option<&File>,
        _mapping: &mut AddressSpace,
        _pos: bindings::loff_t,
        _len: u32,
        _flags: u32,
        _pagep: *mut *mut Page,
        _fsdata: *mut *mut c_types::c_void,
    ) -> Result {
        Err(Error::EINVAL)
    }

    fn write_end(
        &self,
        _file: Option<&File>,
        _mapping: &mut AddressSpace,
        _pos: bindings::loff_t,
        _len: u32,
        _copied: u32,
        _page: &mut Page,
        _fsdata: *mut c_types::c_void,
    ) -> Result<u32> {
        Err(Error::EINVAL)
    }

    fn dirty_folio(&self, _address_space: &mut AddressSpace, _folio: &mut Folio) -> Result<bool> {
        Err(Error::EINVAL)
    }
}

unsafe extern "C" fn readpage_callback<T: AddressSpaceOperations>(
    file: *mut bindings::file,
    page: *mut bindings::page,
) -> c_types::c_int {
    unsafe {
        let address_space = (*file).f_mapping;
        let a_ops = &*((*address_space).private_data as *const T);
        from_kernel_result! {
            a_ops.readpage(&File::from_ptr(file), &mut (*page)).map(|()| 0)
        }
    }
}

unsafe extern "C" fn write_begin_callback<T: AddressSpaceOperations>(
    file: *mut bindings::file,
    mapping: *mut bindings::address_space,
    pos: bindings::loff_t,
    len: c_types::c_uint,
    flags: c_types::c_uint,
    pagep: *mut *mut Page,
    fsdata: *mut *mut c_types::c_void,
) -> c_types::c_int {
    unsafe {
        let a_ops = &*((*mapping).private_data as *const T);
        let file = (!file.is_null()).then(|| File::from_ptr(file));
        from_kernel_result! {
            a_ops.write_begin(file.as_deref(), &mut (*mapping), pos, len, flags, pagep, fsdata).map(|()| 0)
        }
    }
}

unsafe extern "C" fn write_end_callback<T: AddressSpaceOperations>(
    file: *mut bindings::file,
    mapping: *mut bindings::address_space,
    pos: bindings::loff_t,
    len: c_types::c_uint,
    copied: c_types::c_uint,
    page: *mut bindings::page,
    fsdata: *mut c_types::c_void,
) -> c_types::c_int {
    unsafe {
        let a_ops = &*((*mapping).private_data as *const T);
        let file = (!file.is_null()).then(|| File::from_ptr(file));
        from_kernel_result! {
                a_ops.write_end(file.as_deref(), &mut (*mapping), pos, len, copied, &mut (*page), fsdata).map(|x| x as i32)
        }
    }
}

unsafe extern "C" fn dirty_folio_callback<T: AddressSpaceOperations>(
    address_space: *mut bindings::address_space, folio: *mut bindings::folio
) -> bool {
    unsafe {
        let a_ops = &*((*address_space).private_data as *const T);
        a_ops.dirty_folio(&mut (*address_space), &mut (*folio)).unwrap_or(false)
    }
}

pub(crate) struct AddressSpaceOperationsVtable<T>(marker::PhantomData<T>);

impl<T: AddressSpaceOperations> AddressSpaceOperationsVtable<T> {
    const VTABLE: bindings::address_space_operations = bindings::address_space_operations {
        readpage: if T::TO_USE.readpage {
            Some(readpage_callback::<T>)
        } else {
            None
        },
        write_begin: if T::TO_USE.write_begin {
            Some(write_begin_callback::<T>)
        } else {
            None
        },
        write_end: if T::TO_USE.write_end {
            Some(write_end_callback::<T>)
        } else {
            None
        },
        dirty_folio: if T::TO_USE.dirty_folio {
            Some(dirty_folio_callback::<T>)
        } else {
            None
        },
        writepage: None,
        writepages: None,
        readahead: None,
        bmap: None,
        invalidate_folio: None,
        releasepage: None,
        freepage: None,
        direct_IO: None,
        migratepage: None,
        isolate_page: None,
        putback_page: None,
        launder_folio: None,
        is_partially_uptodate: None,
        is_dirty_writeback: None,
        error_remove_page: None,
        swap_activate: None,
        swap_deactivate: None,

    };
}

impl<T: AddressSpaceOperations> BuildVtable<bindings::address_space_operations>
    for AddressSpaceOperationsVtable<T>
{
    fn build_vtable() -> &'static bindings::address_space_operations {
        &Self::VTABLE
    }
}

impl<T: AddressSpaceOperations> BuildVtable<bindings::address_space_operations> for T {
    fn build_vtable() -> &'static bindings::address_space_operations {
        AddressSpaceOperationsVtable::<T>::build_vtable()
    }
}

/// Represents which fields of [`struct address_space_operation`] should be populated with pointers.
pub struct ToUse {
    /// The `readpage` field of [`struct address_space_operation`].
    pub readpage: bool,
    /// The `write_begin` field of [`struct address_space_operation`].
    pub write_begin: bool,
    /// The `write_begin` field of [`struct address_space_operation`].
    pub write_end: bool,
    /// The `dirty_folio` field of [`struct address_space_operation`].
    pub dirty_folio: bool,
}

/// A constant version where all values are to set to `false`, that is, all supported fields will
/// be set to null pointers.
pub const USE_NONE: ToUse = ToUse {
    readpage: false,
    write_begin: false,
    write_end: false,
    dirty_folio: false,
};

#[macro_export]
macro_rules! declare_address_space_operations {
    () => {
        const TO_USE: $crate::fs::address_space_operations::ToUse = $crate::fs::address_space_operations::USE_NONE;
    };
    ($($i:ident),+) => {
        const TO_USE: kernel::fs::address_space_operations::ToUse =
            $crate::fs::address_space_operations::ToUse {
                $($i: true),+ ,
                ..$crate::fs::address_space_operations::USE_NONE
            };
    };
}
