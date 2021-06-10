use core::ops::{Deref, DerefMut};
use core::{mem, ptr};

use crate::bindings;
use crate::c_types::*;
use crate::fs::SuperBlock;
use crate::types::Mode;

#[derive(PartialEq, Eq)]
pub enum UpdateATime {
    Yes,
    No,
}
#[derive(PartialEq, Eq)]
pub enum UpdateCTime {
    Yes,
    No,
}
#[derive(PartialEq, Eq)]
pub enum UpdateMTime {
    Yes,
    No,
}

#[repr(transparent)]
pub struct Inode(bindings::inode);

impl Inode {
    pub fn as_ptr_mut(&mut self) -> *mut bindings::inode {
        self.deref_mut() as *mut _
    }

    pub fn new(sb: &mut SuperBlock) -> Option<&mut Self> {
        unsafe {
            bindings::new_inode(sb as *mut _)
                .as_mut()
                .map(AsMut::as_mut)
        }
    }

    pub fn next_ino() -> u32 {
        unsafe { bindings::get_next_ino() } // FIXME: why do the bindings not return c_int here?
    }

    pub fn init_owner(
        &mut self,
        ns: &mut bindings::user_namespace,
        directory: Option<&mut Inode>,
        mode: Mode,
    ) {
        unsafe {
            bindings::inode_init_owner(
                ns as *mut _,
                self.as_ptr_mut(),
                directory
                    .map(Inode::as_ptr_mut)
                    .unwrap_or_else(ptr::null_mut),
                mode.as_int(),
            );
        }
    }

    pub fn update_acm_time(&mut self, a: UpdateATime, c: UpdateCTime, m: UpdateMTime) {
        let time = unsafe { bindings::current_time(self.as_ptr_mut()) };
        if a == UpdateATime::Yes {
            self.i_atime = time;
        }
        if c == UpdateCTime::Yes {
            self.i_ctime = time;
        }
        if m == UpdateMTime::Yes {
            self.i_mtime = time;
        }
    }

    pub fn inc_nlink(&mut self) {
        unsafe {
            bindings::inc_nlink(self.as_ptr_mut());
        }
    }
    pub fn nohighmem(&mut self) {
        unsafe {
            bindings::inode_nohighmem(self.as_ptr_mut());
        }
    }
    pub fn init_special(&mut self, mode: Mode, device: bindings::dev_t) {
        unsafe {
            bindings::init_special_inode(self.as_ptr_mut(), mode.as_int(), device);
        }
    }
    pub fn put(&mut self) {
        unsafe {
            bindings::iput(self.as_ptr_mut());
        }
    }
}

impl Deref for Inode {
    type Target = bindings::inode;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Inode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<Inode> for bindings::inode {
    fn as_ref(&self) -> &Inode {
        unsafe { mem::transmute(self) }
    }
}
impl AsMut<Inode> for bindings::inode {
    fn as_mut(&mut self) -> &mut Inode {
        unsafe { mem::transmute(self) }
    }
}
