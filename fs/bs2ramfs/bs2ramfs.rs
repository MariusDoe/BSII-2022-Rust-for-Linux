#![no_std]

use alloc::boxed::Box;
use core::{mem, ptr};

use kernel::{
    bindings,
    c_types::*,
    file::File,
    file_operations::{FileOperations, SeekFrom},
    fs::{
        address_space::AddressSpace,
        address_space_operations::AddressSpaceOperations,
        dentry::Dentry,
        inode::{Inode, UpdateATime, UpdateCTime, UpdateMTime},
        inode_operations::InodeOperations,
        kiocb::Kiocb,
        libfs_functions::{self, PageSymlinkInodeOperations, SimpleDirOperations},
        super_block::SuperBlock,
        super_operations::{Kstatfs, SeqFile, SuperOperations},
        FileSystemBase, FileSystemType,
    },
    iov_iter::IovIter,
    prelude::*,
    print::ExpectK,
    str::CStr,
    types::{Dev, FileSystemFlags, FileSystemFlagsInt, Iattr, Kstat, Page, Path, UserNamespace},
    Error, Mode,
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
    name: b"bs2ramfs",
    author: b"Rust for Linux Contributors",
    description: b"RAMFS",
    license: b"GPL v2",
}

struct BS2Ramfs;

impl FileSystemBase for BS2Ramfs {
    const NAME: &'static CStr = kernel::c_str!("bs2ramfs");
    const FS_FLAGS: FileSystemFlagsInt = FileSystemFlags::FS_USERNS_MOUNT;

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
        let ops = Bs2RamfsSuperOps::default();

        // TODO: investigate if this really has to be set to NULL in case we run out of memory
        sb.s_root = ptr::null_mut();
        sb.s_root = ramfs_get_inode(sb, None, Mode::S_IFDIR | ops.mount_opts.mode, 0)
            .and_then(Dentry::make_root)
            .ok_or(Error::ENOMEM)? as *mut _ as *mut _;
        pr_emerg!("(rust) s_root: {:?}", sb.s_root);

        sb.set_super_operations(ops)?;
        sb.s_maxbytes = MAX_LFS_FILESIZE;
        sb.s_blocksize = kernel::PAGE_SIZE as _;
        sb.s_blocksize_bits = PAGE_SHIFT as _;
        sb.s_time_gran = 1;
        pr_emerg!("SB members set");

        Ok(())
    }
}
kernel::declare_fs_type!(BS2Ramfs, BS2RAMFS_FS_TYPE);

impl KernelModule for BS2Ramfs {
    fn init() -> Result<Self> {
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
    pub mode: Mode,
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

impl FileOperations for Bs2RamfsFileOps {
    kernel::declare_file_operations!(
        read_iter,
        write_iter,
        mmap,
        fsync,
        splice_read,
        splice_write,
        seek
    );

    fn read_iter(&self, iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
        libfs_functions::generic_file_read_iter(iocb, iter)
    }

    fn write_iter(&self, iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
        libfs_functions::generic_file_write_iter(iocb, iter)
    }

    fn mmap(&self, file: &File, vma: &mut bindings::vm_area_struct) -> Result {
        libfs_functions::generic_file_mmap(file, vma)
    }

    fn fsync(&self, file: &File, start: u64, end: u64, datasync: bool) -> Result<u32> {
        libfs_functions::noop_fsync(file, start, end, datasync)
    }

    fn seek(&self, file: &File, pos: SeekFrom) -> Result<u64> {
        libfs_functions::generic_file_llseek(file, pos)
    }

    fn splice_read(
        &self,
        file: &File,
        pos: *mut i64,
        pipe: &mut bindings::pipe_inode_info,
        len: usize,
        flags: u32,
    ) -> Result<usize> {
        libfs_functions::generic_file_splice_read(file, pos, pipe, len, flags)
    }

    fn splice_write(
        &self,
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

    fn drop_inode(&self, inode: &mut Inode) -> Result {
        libfs_functions::generic_delete_inode(inode)
    }

    fn statfs(&self, root: &mut Dentry, buf: &mut Kstatfs) -> Result {
        libfs_functions::simple_statfs(root, buf)
    }

    fn show_options(&self, _s: &mut SeqFile, _root: &mut Dentry) -> Result {
        pr_emerg!("ramfs show options, doing nothing");
        Ok(())
    }
}

#[derive(Default)]
struct Bs2RamfsAOps;

impl AddressSpaceOperations for Bs2RamfsAOps {
    kernel::declare_address_space_operations!(readpage, write_begin, write_end, set_page_dirty);

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

    fn set_page_dirty(&self, page: &mut Page) -> Result<bool> {
        libfs_functions::__set_page_dirty_nobuffers(page)
    }
}

#[derive(Default)]
struct Bs2RamfsFileInodeOps;

impl InodeOperations for Bs2RamfsFileInodeOps {
    kernel::declare_inode_operations!(setattr, getattr);

    fn setattr(
        &self,
        mnt_userns: &mut UserNamespace,
        dentry: &mut Dentry,
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
        dir: &mut Inode,
        dentry: &mut Dentry,
        mode: Mode,
        _excl: bool,
    ) -> Result {
        pr_emerg!("enter create");
        self.mknod(mnt_userns, dir, dentry, mode | Mode::S_IFREG, 0)
    }

    fn lookup(&self, dir: &mut Inode, dentry: &mut Dentry, flags: c_uint) -> Result<*mut Dentry> {
        pr_emerg!("enter lookup");
        libfs_functions::simple_lookup(dir, dentry, flags) // niklas: This returns 0, but it does so on main too, so it's not the problem
    }

    fn link(&self, old_dentry: &mut Dentry, dir: &mut Inode, dentry: &mut Dentry) -> Result {
        libfs_functions::simple_link(old_dentry, dir, dentry)
    }

    fn unlink(&self, dir: &mut Inode, dentry: &mut Dentry) -> Result {
        libfs_functions::simple_unlink(dir, dentry)
    }

    fn symlink(
        &self,
        _mnt_userns: &mut UserNamespace,
        dir: &mut Inode,
        dentry: &mut Dentry,
        symname: &'static CStr,
    ) -> Result {
        let inode = ramfs_get_inode(
            dir.super_block_mut(),
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
        dir: &mut Inode,
        dentry: &mut Dentry,
        mode: Mode,
    ) -> Result {
        pr_emerg!("enter mkdir");
        if let Err(_) = self.mknod(mnt_userns, dir, dentry, mode | Mode::S_IFDIR, 0) {
            pr_emerg!("mkdir: inc_nlink");
            dir.inc_nlink();
        }
        Ok(())
    }

    fn rmdir(&self, dir: &mut Inode, dentry: &mut Dentry) -> Result {
        libfs_functions::simple_rmdir(dir, dentry)
    }

    fn mknod(
        &self,
        _mnt_userns: &mut UserNamespace,
        dir: &mut Inode,
        dentry: &mut Dentry,
        mode: Mode,
        dev: Dev,
    ) -> Result {
        ramfs_get_inode(dir.super_block_mut(), Some(dir), mode, dev)
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
        old_dir: &mut Inode,
        old_dentry: &mut Dentry,
        new_dir: &mut Inode,
        new_dentry: &mut Dentry,
        flags: c_uint,
    ) -> Result {
        libfs_functions::simple_rename(mnt_userns, old_dir, old_dentry, new_dir, new_dentry, flags)
    }
}

pub fn ramfs_get_inode<'a>(
    sb: &'a mut SuperBlock,
    dir: Option<&'_ mut Inode>,
    mode: Mode,
    dev: bindings::dev_t,
) -> Option<&'a mut Inode> {
    Inode::new(sb).map(|inode| {
        inode.i_ino = Inode::next_ino() as _;
        inode.init_owner(unsafe { &mut bindings::init_user_ns }, dir, mode);

        inode
            .mapping_mut()
            .set_address_space_operations(Bs2RamfsAOps)
            .expectk("Set address space operations");

        // TODO: these should be functions on the AddressSpace
        unsafe {
            rust_helper_mapping_set_gfp_mask(inode.i_mapping, RUST_HELPER_GFP_HIGHUSER);
            rust_helper_mapping_set_unevictable(inode.i_mapping);
        }

        inode.update_acm_time(UpdateATime::Yes, UpdateCTime::Yes, UpdateMTime::Yes);
        match mode & Mode::S_IFMT {
            Mode::S_IFREG => {
                inode.set_inode_operations(Bs2RamfsFileInodeOps);
                inode.set_file_operations::<Bs2RamfsFileOps>();
            }
            Mode::S_IFDIR => {
                inode.set_inode_operations(Bs2RamfsDirInodeOps);
                inode.set_file_operations::<SimpleDirOperations>();
                inode.inc_nlink();
            }
            Mode::S_IFLNK => {
                inode.set_inode_operations(PageSymlinkInodeOperations);
                inode.nohighmem();
            }
            _ => {
                inode.init_special(mode, dev);
            }
        }

        inode
    })
}
