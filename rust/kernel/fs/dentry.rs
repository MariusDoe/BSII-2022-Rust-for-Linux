use core::mem;
use core::ops::{Deref, DerefMut};

use crate::bindings;
use crate::fs::inode::Inode;

extern "C" {
    fn rust_helper_dget(dentry: *mut bindings::dentry);
}

#[repr(transparent)]
pub struct Dentry(bindings::dentry);

impl Deref for Dentry {
    type Target = bindings::dentry;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Dentry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<Dentry> for bindings::dentry {
    fn as_ref(&self) -> &Dentry {
        unsafe { mem::transmute(self) }
    }
}
impl AsMut<Dentry> for bindings::dentry {
    fn as_mut(&mut self) -> &mut Dentry {
        unsafe { mem::transmute(self) }
    }
}

impl Dentry {
    pub fn as_ptr_mut(&mut self) -> *mut bindings::dentry {
        self.deref_mut() as *mut _
    }

    pub fn make_root(inode: &mut Inode) -> Option<&mut Self> {
        unsafe { (bindings::d_make_root(inode.as_ptr_mut()) as *mut Self).as_mut() }
    }

    pub fn lookup(&mut self, query: *const bindings::qstr) -> Option<&mut Self> {
        unsafe { (bindings::d_lookup(self.as_ptr_mut(), query) as *mut Self).as_mut() }
    }

    pub fn get(&mut self) {
        // Note: while the original `dget` function also allows NULL as an argument, it doesn't do
        // anything with it, so only wrapping the function for non-null pointers should be okay.
        unsafe { rust_helper_dget(self.as_ptr_mut()) };
    }

    pub fn put(&mut self) {
        unsafe {
            bindings::dput(self.as_ptr_mut());
        }
    }

    pub fn drop_dentry(&mut self) {
        unsafe {
            bindings::d_drop(self.as_ptr_mut());
        }
    }

    pub fn delete_dentry(&mut self) {
        unsafe {
            bindings::d_delete(self.as_ptr_mut());
        }
    }

    pub fn add(&mut self, inode: &mut Inode) {
        unsafe {
            bindings::d_add(self.as_ptr_mut(), inode.as_ptr_mut());
        }
    }

    pub fn instantiate(&mut self, inode: &mut Inode) {
        unsafe {
            bindings::d_instantiate(self.as_ptr_mut(), inode.as_ptr_mut());
        }
    }
}
