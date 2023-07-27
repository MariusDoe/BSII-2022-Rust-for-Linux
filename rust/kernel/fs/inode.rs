#![allow(unused_unsafe)]

use core::ops::{Deref, DerefMut};
use core::{mem, ptr};

use crate::{
    bindings,
    c_types::*,
    error::Error,
    fs::super_block::SuperBlock,
    fs::BuildVtable,
    print::ExpectK,
    types::{AddressSpace, Dev, Mode},
    Result,
};

extern "C" {
    fn rust_helper_inode_lock(inode: *mut bindings::inode);
    fn rust_helper_inode_unlock(inode: *mut bindings::inode);
    fn rust_helper_mark_inode_dirty(inode: *mut bindings::inode);
    fn rust_helper_i_size_read(inode: *const bindings::inode) -> bindings::loff_t;
    fn rust_helper_insert_inode_hash(inode: *mut bindings::inode);
    fn rust_helper_inode_set_iversion(inode: *mut bindings::inode, value: u64);
    fn rust_helper_inode_inc_iversion(inode: *mut bindings::inode);
}

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
#[derive(PartialEq, Eq)]
pub enum WriteSync {
    Yes,
    No,
}

#[repr(transparent)]
pub struct BS2InodeInfo(bindings::msdos_inode_info);

#[repr(transparent)]
pub struct Inode(bindings::inode);

impl Inode {
    pub fn as_ptr(&self) -> *const bindings::inode {
        self.deref() as *const _
    }

    pub fn as_ptr_mut(&mut self) -> *mut bindings::inode {
        self.deref_mut() as *mut _
    }

    pub fn new(sb: &mut SuperBlock) -> Option<&mut Self> {
        unsafe {
            let mut inode = bindings::new_inode(sb.as_ptr_mut())
                .as_mut()
                .map(AsMut::as_mut);
            if let Some(inode) = inode.as_mut() {
                inode.i_private = crate::container_of!(inode, bindings::msdos_inode_info, vfs_inode) as *mut _;

            }
            
            
            
            inode
        }
    }

    pub fn get_info(&mut self) -> Option<&mut BS2InodeInfo> {
        unsafe { 
            (self.i_private as *mut bindings::msdos_inode_info)
            .as_mut()
            .map(AsMut::as_mut)
        
        }
    }

    pub fn mode(&self) -> Mode {
        Mode::from_int(self.i_mode)
    }

    #[inline]
    pub fn super_block(&self) -> &SuperBlock {
        unsafe {
            self.i_sb
                .as_mut()
                .expectk("Inode had NULL super block")
                .as_mut()
        }
    }
    #[inline]
    pub fn super_block_mut<'this, 'ret>(&'this mut self) -> &'ret mut SuperBlock {
        unsafe {
            self.i_sb
                .as_mut()
                .expectk("Inode had NULL super block")
                .as_mut()
        }
    }

    pub fn next_ino() -> u32 {
        unsafe { bindings::get_next_ino() } // FIXME: why do the bindings not return c_int here?
    }

    pub fn mapping(&mut self) -> &mut AddressSpace {
        unsafe { self.i_mapping.as_mut().expectk("Inode had NULL mapping") }
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

    pub fn is_sync(&self) -> bool {
        // TODO: proper wrappers
        const S_SYNC: u64 = 1;
        const SB_SYNCHRONOUS: u64 = 16;
        ((self.i_flags as u64 & S_SYNC) | (self.super_block().s_flags as u64 & SB_SYNCHRONOUS)) != 0
    }

    pub fn mark_dirty(&mut self) {
        unsafe {
            rust_helper_mark_inode_dirty(self.as_ptr_mut());
        }
    }

    pub fn write_now(&mut self, sync: WriteSync) -> Result {
        let sync = (sync == WriteSync::Yes) as c_int;
        Error::parse_int(unsafe { bindings::write_inode_now(self.as_ptr_mut(), sync) }).map(|_| ())
    }

    pub fn current_time(&mut self) -> bindings::timespec64 {
        unsafe { bindings::current_time(self.as_ptr_mut()) }
    }

    pub fn update_acm_time(&mut self, a: UpdateATime, c: UpdateCTime, m: UpdateMTime) {
        let time = self.current_time();
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
    pub fn init_special(&mut self, mode: Mode, device: Dev) {
        unsafe {
            bindings::init_special_inode(self.as_ptr_mut(), mode.as_int(), device);
        }
    }
    pub fn put(&mut self) {
        unsafe {
            bindings::iput(self.as_ptr_mut());
        }
    }

    pub fn set_file_operations<V: BuildVtable<bindings::file_operations>>(&mut self) {
        self.__bindgen_anon_3.i_fop = V::build_vtable();
    }

    pub fn set_inode_operations<Ops: BuildVtable<bindings::inode_operations>>(&mut self, ops: &'static Ops) {
        self.i_op = Ops::build_vtable();
        self.i_private = ops as *const _ as *mut _;
    }

    // I think Inode should rather have a method get_address_space, and the AddressSpace should then provide set_address_space_operations
    pub fn set_address_space_operations<Ops: BuildVtable<bindings::address_space_operations>>(
        &mut self,
        ops: &'static Ops,
    ) {
        unsafe {
            (*self.i_mapping).a_ops = Ops::build_vtable();
            (*self.i_mapping).private_data = ops as *const _ as *mut _;
        }
    }

    pub fn size_read(&self) -> bindings::loff_t {
        unsafe { rust_helper_i_size_read(self.as_ptr()) }
    }

    pub fn lock(&mut self) -> LockGuard<'_> {
        LockGuard::new(self)
    }

    pub fn insert_hash(&mut self) {
        unsafe {
            rust_helper_insert_inode_hash(self.as_ptr_mut());
        }
    }

    pub fn set_iversion(&mut self, value: u64) {
        unsafe {
            rust_helper_inode_set_iversion(self.as_ptr_mut(), value);
        }
    }

    pub fn inc_iversion(&mut self) {
        unsafe {
            rust_helper_inode_inc_iversion(self.as_ptr_mut());
        }
    }
    
    pub fn set_i_fop (&mut self, value: *const bindings::file_operations) {
        unsafe {
            self.0.__bindgen_anon_3.i_fop = value;
        }
    }
}

pub struct LockGuard<'a> {
    inner: &'a mut Inode,
}

impl<'a> LockGuard<'a> {
    pub fn new(inner: &'a mut Inode) -> Self {
        unsafe {
            rust_helper_inode_lock(inner.as_ptr_mut());
        }
        Self { inner }
    }
}

impl<'a> Drop for LockGuard<'a> {
    fn drop(&mut self) {
        unsafe {
            rust_helper_inode_unlock(self.inner.as_ptr_mut());
        }
    }
}
impl<'a> Deref for LockGuard<'a> {
    type Target = Inode;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> DerefMut for LockGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
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

impl AsRef<BS2InodeInfo> for bindings::msdos_inode_info {
    fn as_ref(&self) -> &BS2InodeInfo {
        unsafe { mem::transmute(self) }
    }
}

impl AsMut<BS2InodeInfo> for bindings::msdos_inode_info {
    fn as_mut(&mut self) -> &mut BS2InodeInfo {
        unsafe { mem::transmute(self) }
    }
}

impl Deref for BS2InodeInfo {
    type Target = bindings::msdos_inode_info;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BS2InodeInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}