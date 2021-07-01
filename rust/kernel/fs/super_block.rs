use alloc::boxed::Box;
use core::{
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::{
    bindings,
    buffer_head::BufferHead,
    c_types::*,
    fs::super_operations::{SuperOperations, SuperOperationsVtable},
    Result,
};

extern "C" {
    fn rust_helper_sb_bread(
        sb: *mut bindings::super_block,
        block: bindings::sector_t,
    ) -> *mut bindings::buffer_head;
}

#[repr(transparent)]
pub struct SuperBlock(bindings::super_block);

impl SuperBlock {
    pub fn as_ptr_mut(&mut self) -> *mut bindings::super_block {
        self.deref_mut() as *mut _
    }

    pub fn set_super_operations<OPS: SuperOperations>(&mut self, ops: OPS) -> Result {
        self.s_op = unsafe { SuperOperationsVtable::<OPS>::build() };
        self.s_fs_info = Box::into_raw(Box::try_new(ops)?).cast();
        Ok(())
    }

    pub fn take_super_operations<Ops: SuperOperations>(&mut self) -> Option<Box<Ops>> {
        self.s_op = ptr::null_mut();
        let p = mem::replace(&mut self.s_fs_info, ptr::null_mut()).cast::<Ops>();
        if p.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(p) })
        }
    }

    /// Returns the blocksize that is chosen
    pub fn set_min_blocksize(&mut self, size: i32) -> c_int {
        unsafe { bindings::sb_min_blocksize(self.as_ptr_mut(), size) }
    }

    pub fn set_blocksize(&mut self, size: i32) -> c_int {
        unsafe { bindings::sb_set_blocksize(self.as_ptr_mut(), size) }
    }

    /// The returned buffer should be discarded using `libfs_functions::release_buffer` (otherwise
    /// known as `brelse`).
    #[must_use]
    pub fn read_block<'this, 'ret>(&'this mut self, block: u64) -> Option<&'ret mut BufferHead> {
        unsafe { rust_helper_sb_bread(self.as_ptr_mut(), block).as_mut() }.map(AsMut::as_mut)
    }
}

impl Deref for SuperBlock {
    type Target = bindings::super_block;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for SuperBlock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<SuperBlock> for bindings::super_block {
    fn as_ref(&self) -> &SuperBlock {
        unsafe { mem::transmute(self) }
    }
}
impl AsMut<SuperBlock> for bindings::super_block {
    fn as_mut(&mut self) -> &mut SuperBlock {
        unsafe { mem::transmute(self) }
    }
}
