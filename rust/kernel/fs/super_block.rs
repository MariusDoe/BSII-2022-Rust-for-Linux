use core::mem;
use core::ops::{Deref, DerefMut};

use crate::bindings;
use crate::fs::super_operations::{SuperOperations, SuperOperationsVtable};

#[repr(transparent)]
pub struct SuperBlock(bindings::super_block);

impl SuperBlock {
    pub fn as_ptr_mut(&mut self) -> *mut bindings::super_block {
        self.deref_mut() as *mut _
    }

    pub fn set_super_operations<OPS: SuperOperations>(&mut self) {
        self.s_op = unsafe { SuperOperationsVtable::<OPS>::build() };
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
