use kernel::fs::dentry::Dentry;
use kernel::fs::inode::Inode;
use kernel::c_types::c_uint;
use kernel::types::{UserNamespace, Dev, Mode};
use kernel::prelude::*;
use kernel::fs::inode_operations::InodeOperations;


#[derive(Default)]
pub(crate) struct BS2FatDirInodeOps;

#[allow(unused_variables)]
impl InodeOperations for BS2FatDirInodeOps {
    kernel::declare_inode_operations!(
        create, lookup, link, unlink, symlink, mkdir, rmdir, mknod, rename
    );

    fn create(
        &self,
        mnt_userns: &mut UserNamespace,
        dir: &mut Inode,
        dentry: &mut Dentry,
        mode: Mode,
        _excl: bool,
    ) -> Result {
        todo!("create")
    }

    fn lookup(&self, dir: &mut Inode, dentry: &mut Dentry, flags: c_uint) -> Result<*mut Dentry> {
        todo!("lookup")
    }

    fn link(&self, old_dentry: &mut Dentry, dir: &mut Inode, dentry: &mut Dentry) -> Result {
        todo!("link")
    }

    fn unlink(&self, dir: &mut Inode, dentry: &mut Dentry) -> Result {
        todo!("unlink")
    }

    fn symlink(
        &self,
        _mnt_userns: &mut UserNamespace,
        dir: &mut Inode,
        dentry: &mut Dentry,
        symname: &'static CStr,
    ) -> Result {
        todo!("symlink")
    }

    fn mkdir(
        &self,
        mnt_userns: &mut UserNamespace,
        dir: &mut Inode,
        dentry: &mut Dentry,
        mode: Mode,
    ) -> Result {
        todo!("mkdir")
    }

    fn rmdir(&self, dir: &mut Inode, dentry: &mut Dentry) -> Result {
        todo!("rmdir")
    }

    fn mknod(
        &self,
        _mnt_userns: &mut UserNamespace,
        dir: &mut Inode,
        dentry: &mut Dentry,
        mode: Mode,
        dev: Dev,
    ) -> Result {
        todo!("mknod")
    }
    fn rename(
        &self,
        mnt_userns: &mut UserNamespace,
        old_dir: &mut Inode,
        old_dentry: &mut Dentry,
        new_dir: &mut Inode,
        new_dentry: &mut Dentry,
        flags: c_uint,
    ) -> Result {
        todo!("rename")
    }
}

/* #[derive(Default)]
pub(crate) struct BS2FatDirOps;

impl Operations for BS2FatDirOps {
    kernel::declare_file_operations!(
        seek, read, ioctl, compat_ioctl, fsync
    );

    fn seek(&self, file: &mut File, offset: loff_t, whence: c_uint) -> Result<loff_t> {
        todo!("llseek")
    }

    fn read(&self, file: &mut File, buf: &mut [u8], count: size_t, pos: loff_t) -> Result<ssize_t> {
        todo!("read")
    }

    fn iterate_shared(
        &self,
        file: &mut File,
        pos: loff_t,
        ppos: &mut loff_t,
        f: &mut dyn FnMut(&mut Dentry) -> bool,
    ) -> Result<loff_t> {
        todo!("iterate_shared")
    }

    fn unlocked_ioctl(
        &self,
        file: &mut File,
        cmd: c_uint,
        arg: ulong,
        _mnt_userns: &mut UserNamespace,
    ) -> Result<long> {
        todo!("unlocked_ioctl")
    }

    fn compat_ioctl(
            _data: <Self::Data as kernel::types::PointerWrapper>::Borrowed<'_>,
            _file: &kernel::file::File,
            _cmd: &mut kernel::file::IoctlCommand,
        ) -> Result<i32> {
            todo!("compat_ioctl")
    }

    fn fsync(&self, file: &mut File, start: loff_t, end: loff_t, datasync: bool) -> Result {
        todo!("fsync")
    }
} */