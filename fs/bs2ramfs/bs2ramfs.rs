// TODO: Remove allows
#![allow(missing_docs)]
#![allow(improper_ctypes)]

use alloc::boxed::Box;
use core::{mem, ptr};

use core::ffi::{c_int, c_longlong, c_uint, c_void};
use kernel::{
    bindings,
    file::{File, Operations, SeekFrom},
    fs::{
        // address_space_operations::AddressSpaceOperations,
        DEntry,
        INode,
       // inode::{UpdateATime, UpdateCTime, UpdateMTime},
       // inode_operations::InodeOperations,
       // kiocb::Kiocb,
       // libfs_functions::{self, PageSymlinkInodeOperations, SimpleDirOperations},
        SuperBlock,
       // super_operations::{Kstatfs, SeqFile, SuperOperations},
       // FileSystemBase, FileSystemType,
    },
    iov_iter::IovIter,
    mm,
    prelude::*,
    str::CStr,
    // types::{AddressSpace, Dev, Folio, Iattr, Kstat, Page, Path, UserNamespace},
    // Error, Mode,
    Module,
};

const PAGE_SHIFT: u32 = 12; // x86 (maybe)
const MAX_LFS_FILESIZE: c_longlong = c_longlong::MAX;
const BS2RAMFS_MAGIC: u64 = 0x858458f6; // ~~one less than~~ ramfs, should not clash with anything (maybe)

extern "C" {
    fn rust_helper_mapping_set_unevictable(mapping: *mut bindings::address_space);
    fn rust_helper_mapping_set_gfp_mask(
        mapping: *mut bindings::address_space,
        mask: bindings::gfp_t,
    );
    static RUST_HELPER_GFP_HIGHUSER: bindings::gfp_t;
}

module! {
    type: BS2Ramfs,
    name: "bs2ramfs",
    author: "Rust for Linux Contributors",
    description: "RAMFS",
    license: "GPL v2",
}

struct BS2Ramfs;

impl FileSystemBase for BS2Ramfs {
    const NAME: &'static CStr = kernel::c_str!("bs2ramfs_name");
    const FS_FLAGS: c_int = bindings::FS_USERNS_MOUNT as _;
    const OWNER: *mut bindings::module = ptr::null_mut();

    fn mount(
        _fs_type: &'_ mut FileSystemType,
        flags: c_int,
        _device_name: &CStr,
        data: Option<&mut Self::MountOptions>,
    ) -> Result<*mut bindings::dentry> {
        libfs_functions::mount_nodev::<Self>(flags, data)
    }

    fn kill_super(sb: &mut SuperBlock) {
        let _ = unsafe { Box::from_raw(mem::replace(&mut sb.s_fs_info, ptr::null_mut())) };
        libfs_functions::kill_litter_super(sb);
    }

    fn fill_super(
        sb: &mut SuperBlock,
        _data: Option<&mut Self::MountOptions>,
        _silent: c_int,
    ) -> Result {
        pr_emerg!("Reached ramfs_fill_super_impl");

        sb.s_magic = BS2RAMFS_MAGIC;
        let ops = Box::try_new(Bs2RamfsSuperOps::default())?;

        // TODO: investigate if this really has to be set to NULL in case we run out of memory
        sb.s_root = ptr::null_mut();
        sb.s_root = ramfs_get_inode(sb, None, Mode::S_IFDIR | ops.mount_opts.mode, 0)
            .and_then(DEntry::make_root)
            .ok_or(Error::ENOMEM)? as *mut _ as *mut _;
        pr_emerg!("(rust) s_root: {:?}", sb.s_root);

        let ops = Box::leak(ops);
        sb.set_super_operations(ops);
        sb.s_maxbytes = MAX_LFS_FILESIZE;
        sb.s_blocksize = kernel::PAGE_SIZE as _;
        sb.s_blocksize_bits = PAGE_SHIFT as _;
        sb.s_time_gran = 1;
        pr_emerg!("SB members set");

        Ok(())
    }
}
kernel::declare_fs_type!(BS2Ramfs, BS2RAMFS_FS_TYPE);

impl Module for BS2Ramfs {
    fn init(_name: &'static CStr, _module: &'static ThisModule) -> Result<Self> {
        pr_emerg!("bs2 ramfs in action");
        libfs_functions::register_filesystem::<Self>().map(move |_| Self)
    }
}

impl Drop for BS2Ramfs {
    fn drop(&mut self) {
        let _ = libfs_functions::unregister_filesystem::<Self>();
        pr_info!("bs2 ramfs out of action");
    }
}

struct RamfsMountOpts {
    pub(crate) mode: Mode,
}

impl Default for RamfsMountOpts {
    fn default() -> Self {
        Self {
            mode: Mode::from_int(0o775),
        }
    }
}

#[derive(Default)]
struct Bs2RamfsFileOps;

impl Operations for Bs2RamfsFileOps {
    kernel::declare_file_operations!(
        read_iter,
        write_iter,
        mmap,
        fsync,
        splice_read,
        splice_write,
        seek,
        get_unmapped_area
    );

    fn open(_context: &(), _file: &File) -> Result<Self::Data> {
        Ok(())
    }

    fn read_iter(_data: (), iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
        libfs_functions::generic_file_read_iter(iocb, iter)
    }

    fn write_iter(_data: (), iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
        libfs_functions::generic_file_write_iter(iocb, iter)
    }

    fn mmap(_data: (), file: &File, vma: &mut mm::virt::Area) -> Result {
        libfs_functions::generic_file_mmap(file, vma)
    }

    fn fsync(_data: (), file: &File, start: u64, end: u64, datasync: bool) -> Result<u32> {
        libfs_functions::noop_fsync(file, start, end, datasync)
    }

    fn get_unmapped_area(
        _data: (),
        _file: &File,
        _addr: u64,
        _len: u64,
        _pgoff: u64,
        _flags: u64,
    ) -> Result<u64> {
        pr_emerg!(
            "AKAHSDkADKHAKHD WE ARE ABOUT TO PANIC (IN MMU_GET_UNMAPPED_AREA;;;; LOOK HERE COME ON"
        );
        unimplemented!()
    }

    fn seek(_data: (), file: &File, pos: SeekFrom) -> Result<u64> {
        libfs_functions::generic_file_llseek(file, pos)
    }

    fn splice_read(
        _data: (),
        file: &File,
        pos: *mut i64,
        pipe: &mut bindings::pipe_inode_info,
        len: usize,
        flags: u32,
    ) -> Result<usize> {
        libfs_functions::generic_file_splice_read(file, pos, pipe, len, flags)
    }

    fn splice_write(
        _data: (),
        pipe: &mut bindings::pipe_inode_info,
        file: &File,
        pos: *mut i64,
        len: usize,
        flags: u32,
    ) -> Result<usize> {
        libfs_functions::iter_file_splice_write(pipe, file, pos, len, flags)
    }
}

#[derive(Default)]
struct Bs2RamfsSuperOps {
    mount_opts: RamfsMountOpts,
}

impl SuperOperations for Bs2RamfsSuperOps {
    kernel::declare_super_operations!(statfs, drop_inode, show_options);

    fn drop_inode(&self, inode: &mut INode) -> Result {
        libfs_functions::generic_delete_inode(inode)
    }

    fn statfs(&self, root: &mut DEntry, buf: &mut Kstatfs) -> Result {
        libfs_functions::simple_statfs(root, buf)
    }

    fn show_options(&self, _s: &mut SeqFile, _root: &mut DEntry) -> Result {
        pr_emerg!("ramfs show options, doing nothing");
        Ok(())
    }
}

#[derive(Default)]
struct Bs2RamfsAOps;

impl AddressSpaceOperations for Bs2RamfsAOps {
    kernel::declare_address_space_operations!(readpage, write_begin, write_end, dirty_folio);

    fn readpage(&self, file: &File, page: &mut Page) -> Result {
        libfs_functions::simple_readpage(file, page)
    }

    fn write_begin(
        &self,
        file: Option<&File>,
        mapping: &mut AddressSpace,
        pos: bindings::loff_t,
        len: u32,
        flags: u32,
        pagep: *mut *mut Page,
        fsdata: *mut *mut c_void,
    ) -> Result {
        libfs_functions::simple_write_begin(file, mapping, pos, len, flags, pagep, fsdata)
    }

    fn write_end(
        &self,
        file: Option<&File>,
        mapping: &mut AddressSpace,
        pos: bindings::loff_t,
        len: u32,
        copied: u32,
        page: &mut Page,
        fsdata: *mut c_void,
    ) -> Result<u32> {
        libfs_functions::simple_write_end(file, mapping, pos, len, copied, page, fsdata)
    }

    fn dirty_folio(&self, address_space: &mut AddressSpace, folio: &mut Folio) -> Result<bool> {
        Ok(libfs_functions::filemap_dirty_folio(address_space, folio))
    }
}

#[derive(Default)]
struct Bs2RamfsFileInodeOps;

impl InodeOperations for Bs2RamfsFileInodeOps {
    kernel::declare_inode_operations!(setattr, getattr);

    fn setattr(
        &self,
        mnt_userns: &mut UserNamespace,
        dentry: &mut DEntry,
        iattr: &mut Iattr,
    ) -> Result {
        libfs_functions::simple_setattr(mnt_userns, dentry, iattr)
    }

    fn getattr(
        &self,
        mnt_userns: &mut UserNamespace,
        path: &Path,
        stat: &mut Kstat,
        request_mask: u32,
        query_flags: u32,
    ) -> Result {
        libfs_functions::simple_getattr(mnt_userns, path, stat, request_mask, query_flags)
    }
}

#[derive(Default)]
struct Bs2RamfsDirInodeOps;

impl InodeOperations for Bs2RamfsDirInodeOps {
    kernel::declare_inode_operations!(
        create, lookup, link, unlink, symlink, mkdir, rmdir, mknod, rename
    );

    fn create(
        &self,
        mnt_userns: &mut UserNamespace,
        dir: &mut INode,
        dentry: &mut DEntry,
        mode: Mode,
        _excl: bool,
    ) -> Result {
        pr_emerg!("enter create");
        self.mknod(mnt_userns, dir, dentry, mode | Mode::S_IFREG, 0)
    }

    fn lookup(&self, dir: &mut INode, dentry: &mut DEntry, flags: c_uint) -> Result<*mut DEntry> {
        pr_emerg!("enter lookup");
        libfs_functions::simple_lookup(dir, dentry, flags) // niklas: This returns 0, but it does so on main too, so it's not the problem
    }

    fn link(&self, old_dentry: &mut DEntry, dir: &mut INode, dentry: &mut DEntry) -> Result {
        libfs_functions::simple_link(old_dentry, dir, dentry)
    }

    fn unlink(&self, dir: &mut INode, dentry: &mut DEntry) -> Result {
        libfs_functions::simple_unlink(dir, dentry)
    }

    fn symlink(
        &self,
        _mnt_userns: &mut UserNamespace,
        dir: &mut INode,
        dentry: &mut DEntry,
        symname: &'static CStr,
    ) -> Result {
        let inode = ramfs_get_inode(
            unsafe { dir.i_sb.as_mut().unwrap().as_mut() },
            Some(dir),
            Mode::S_IFLNK | Mode::S_IRWXUGO,
            0,
        )
        .ok_or(Error::ENOSPC)?;

        if let Err(e) = libfs_functions::page_symlink(inode, symname) {
            inode.put();
            return Err(e);
        }

        dentry.instantiate(inode);
        dentry.get();
        dir.update_acm_time(UpdateATime::No, UpdateCTime::Yes, UpdateMTime::Yes);
        Ok(())
    }

    fn mkdir(
        &self,
        mnt_userns: &mut UserNamespace,
        dir: &mut INode,
        dentry: &mut DEntry,
        mode: Mode,
    ) -> Result {
        pr_emerg!("enter mkdir");
        if let Err(_) = self.mknod(mnt_userns, dir, dentry, mode | Mode::S_IFDIR, 0) {
            pr_emerg!("mkdir: inc_nlink");
            dir.inc_nlink();
        }
        Ok(())
    }

    fn rmdir(&self, dir: &mut INode, dentry: &mut DEntry) -> Result {
        libfs_functions::simple_rmdir(dir, dentry)
    }

    fn mknod(
        &self,
        _mnt_userns: &mut UserNamespace,
        dir: &mut INode,
        dentry: &mut DEntry,
        mode: Mode,
        dev: Dev,
    ) -> Result {
        // todo: write some kind of wrapper
        ramfs_get_inode(
            unsafe { dir.i_sb.as_mut().unwrap().as_mut() },
            Some(dir),
            mode,
            dev,
        )
        .ok_or(Error::ENOSPC)
        .map(|inode| {
            dentry.instantiate(inode);
            dentry.get();
            dir.update_acm_time(UpdateATime::No, UpdateCTime::Yes, UpdateMTime::Yes);
            ()
        })
    }
    fn rename(
        &self,
        mnt_userns: &mut UserNamespace,
        old_dir: &mut INode,
        old_dentry: &mut DEntry,
        new_dir: &mut INode,
        new_dentry: &mut DEntry,
        flags: c_uint,
    ) -> Result {
        libfs_functions::simple_rename(mnt_userns, old_dir, old_dentry, new_dir, new_dentry, flags)
    }
}

pub fn ramfs_get_inode<'a>(
    sb: &'a mut SuperBlock,
    dir: Option<&'_ mut INode>,
    mode: Mode,
    dev: bindings::dev_t,
) -> Option<&'a mut INode> {
    INode::new(sb).map(|inode| {
        inode.i_ino = INode::next_ino() as _;
        inode.init_owner(unsafe { &mut bindings::init_user_ns }, dir, mode);

        static A_OPS: Bs2RamfsAOps = Bs2RamfsAOps;
        inode.set_address_space_operations(&A_OPS);

        // I think these should be functions on the AddressSpace, i.e. sth like inode.get_address_space().set_gfp_mask(...)
        unsafe {
            rust_helper_mapping_set_gfp_mask(inode.i_mapping, RUST_HELPER_GFP_HIGHUSER);
            rust_helper_mapping_set_unevictable(inode.i_mapping);
        }

        inode.update_acm_time(UpdateATime::Yes, UpdateCTime::Yes, UpdateMTime::Yes);
        match mode & Mode::S_IFMT {
            Mode::S_IFREG => {
                static I_OPS: Bs2RamfsFileInodeOps = Bs2RamfsFileInodeOps;
                inode.set_inode_operations(&I_OPS);
                inode.set_file_operations::<Bs2RamfsFileOps>();
            }
            Mode::S_IFDIR => {
                static I_OPS: Bs2RamfsDirInodeOps = Bs2RamfsDirInodeOps;
                inode.set_inode_operations(&I_OPS);
                inode.set_file_operations::<SimpleDirOperations>();
                inode.inc_nlink();
            }
            Mode::S_IFLNK => {
                static I_OPS: PageSymlinkInodeOperations = PageSymlinkInodeOperations;
                inode.set_inode_operations(&I_OPS);
                inode.nohighmem();
            }
            _ => {
                inode.init_special(mode, dev);
            }
        }

        inode
    })
}
