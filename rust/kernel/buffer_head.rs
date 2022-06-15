//! Since `bindgen` generates `struct buffer_head` as a ZST, we redefine all the fields here...

use core::mem;

use crate::{bindings, c_types::*};

#[repr(C)]
pub struct BufferHead {
    pub b_state: c_ulong,
    pub b_this_page: *mut BufferHead,
    pub b_page: *mut bindings::page,

    pub b_blocknr: bindings::sector_t,
    pub b_size: c_size_t,
    pub b_data: *mut c_char,

    pub b_bdev: *mut bindings::block_device,
    // FIXME: this is really a function pointer type, but I think we don't need it anyways
    pub b_end_io: *mut c_void,
    pub b_private: *mut c_void,
    pub b_assoc_buffers: bindings::list_head,
    pub b_assoc_map: *mut bindings::address_space,

    pub b_count: bindings::atomic_t,
    pub b_uptodate_lock: bindings::spinlock_t,
}

impl BufferHead {
    pub fn as_ptr_mut(&mut self) -> *mut bindings::buffer_head {
        self as *mut _ as *mut _
    }
}

impl AsRef<BufferHead> for bindings::buffer_head {
    fn as_ref(&self) -> &BufferHead {
        unsafe { mem::transmute(self) }
    }
}
impl AsMut<BufferHead> for bindings::buffer_head {
    fn as_mut(&mut self) -> &mut BufferHead {
        unsafe { mem::transmute(self) }
    }
}
