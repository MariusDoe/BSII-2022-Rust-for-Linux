use core::cell::UnsafeCell;
use core::mem;
use core::ops::{Deref, DerefMut};

use crate::bindings;
use crate::file::File;

#[repr(transparent)]
pub struct Kiocb(bindings::kiocb);

impl Deref for Kiocb {
    type Target = bindings::kiocb;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Kiocb {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<Kiocb> for bindings::kiocb {
    fn as_ref(&self) -> &Kiocb {
        unsafe { mem::transmute(self) }
    }
}
impl AsMut<Kiocb> for bindings::kiocb {
    fn as_mut(&mut self) -> &mut Kiocb {
        unsafe { mem::transmute(self) }
    }
}

impl Kiocb {
    pub fn as_ptr_mut(&mut self) -> *mut bindings::kiocb {
        self.deref_mut() as *mut _
    }

    pub fn get_file(&mut self) -> File {
        unsafe {
            let file = (*(self.as_ptr_mut())).ki_filp;
            // TODO: check whether this is safe
            File(UnsafeCell::new(*file))
        }
    }

    pub fn get_offset(&mut self) -> u64 {
        let offset = unsafe { (*(self.as_ptr_mut())).ki_pos };
        offset as _
    }

    pub fn set_offset(&mut self, offset: u64) {
        unsafe {
            (*(self.as_ptr_mut())).ki_pos = offset as _;
        }
    }
}
