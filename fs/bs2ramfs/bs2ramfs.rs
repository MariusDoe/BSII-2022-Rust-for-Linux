// TODO: Remove allows
#![allow(missing_docs)]
#![allow(improper_ctypes)]

use core::ffi::{c_longlong, c_void};

use kernel::{
    bindings,
    file::{
        AddressSpace, AddressSpaceOperations, AddressSpaceOperationsVtable, File, Folio,
        Operations, Page,
    },
    fs::{
        DEntry,
        Dev,
        EmptyContext,
        // super_operations::{Kstatfs, SeqFile, SuperOperations},
        // FileSystemBase, FileSystemType,
        INode,
        INodeOperations,
        INodeOperationsVtable,
        INodeParams,
        IdMap,
        NewSuperBlock,
        Super,
        // inode::{UpdateATime, UpdateCTime, UpdateMTime},
        // inode_operations::InodeOperations,
        // kiocb::Kiocb,
        // libfs_functions::{self, PageSymlinkInodeOperations, SimpleDirOperations},
        SuperBlock,
        SuperParams,
        Type,
    },
    module_fs,
    prelude::*,
    str::CStr,
};

const PAGE_SHIFT: u32 = 12; // x86 (maybe)
const MAX_LFS_FILESIZE: c_longlong = c_longlong::MAX;
const BS2RAMFS_MAGIC: u32 = 0x858458f6; // ~~one less than~~ ramfs, should not clash with anything (maybe)

extern "C" {
    fn rust_helper_mapping_set_unevictable(mapping: *mut bindings::address_space);
    fn rust_helper_mapping_set_gfp_mask(
        mapping: *mut bindings::address_space,
        mask: bindings::gfp_t,
    );
    static RUST_HELPER_GFP_HIGHUSER: bindings::gfp_t;
}

module_fs! {
    type: BS2Ramfs,
    name: "bs2ramfs",
    author: "Rust for Linux Contributors",
    description: "RAMFS",
    license: "GPL v2",
}

struct BS2Ramfs {
    reg: kernel::fs::Registration,
}

impl Type for BS2Ramfs {
    const NAME: &'static CStr = kernel::c_str!("bs2ramfs_name");
    const FLAGS: i32 = bindings::FS_USERNS_MOUNT as _;
    const SUPER_TYPE: Super = Super::Independent;
    type INodeData = ();
    type Context = EmptyContext;

    fn fill_super(_data: (), sb: NewSuperBlock<'_, Self>) -> Result<&SuperBlock<Self>> {
        pr_emerg!("Reached ramfs fill_super impl");
        let sb = sb.init(
            (),
            &SuperParams {
                magic: BS2RAMFS_MAGIC,
                ..SuperParams::DEFAULT
            },
        )?;
        let root_inode = sb.try_new_dcache_dir_inode(INodeParams {
            mode: (bindings::S_IFDIR | 0o775) as _,
            ino: 1,
            value: (),
        })?;
        let root = sb.try_new_root_dentry(root_inode)?;
        let sb = sb.init_root(root)?;
        pr_emerg!("SB filled");
        Ok(sb)
    }
}

impl kernel::Module for BS2Ramfs {
    fn init(name: &'static CStr, module: &'static ThisModule) -> Result<Self> {
        let mut bs2ramfs_reg = kernel::fs::Registration::register(name, 0, module)?; // it should unregister itself
        Ok(BS2Ramfs { reg: bs2ramfs_reg })
    }
}

struct RamfsMountOpts {
    pub(crate) mode: u16,
}

impl Default for RamfsMountOpts {
    fn default() -> Self {
        Self { mode: 0o775 }
    }
}

#[derive(Default)]
struct Bs2RamfsFileOps;

#[vtable] // file.rs 570 OperationsVtable
impl Operations for Bs2RamfsFileOps {
    type Data = ();

    fn open(_context: &(), _file: &File) -> Result<Self::Data> {
        Ok(())
    }
}

#[derive(Default)]
struct Bs2RamfsSuperOps {
    mount_opts: RamfsMountOpts,
}

impl SuperOperations for Bs2RamfsSuperOps {
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

#[vtable]
impl AddressSpaceOperations for Bs2RamfsAOps {
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
struct BS2RamfsFileINodeOps;

#[vtable]
impl INodeOperations<BS2Ramfs> for BS2RamfsFileINodeOps {
    // I'm broken, because bindings::simple_* are used :(
}

#[derive(Default)]
struct BS2RamfsDirINodeOps;

#[vtable]
impl INodeOperations<BS2Ramfs> for BS2RamfsDirINodeOps {
    fn create(
        &self,
        mnt_userns: &mut IdMap,
        dir: &mut INode<BS2Ramfs>,
        dentry: &mut DEntry<BS2Ramfs>,
        mode: u16,
        _excl: bool,
    ) -> Result {
        pr_emerg!("enter create");
        self.mknod(mnt_userns, dir, dentry, mode | bindings::S_IFREG as _, 0)
    }

    fn symlink(
        &self,
        _mnt_userns: &mut IdMap,
        dir: &mut INode<BS2Ramfs>,
        dentry: &mut DEntry<BS2Ramfs>,
        symname: &'static CStr,
    ) -> Result {
        let inode = ramfs_get_inode(
            unsafe { dir.i_sb.as_mut().unwrap().as_mut() },
            Some(dir),
            (bindings::S_IFLNK | bindings::S_IRWXUGO) as _,
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
        mnt_userns: &mut IdMap,
        dir: &mut INode<BS2Ramfs>,
        dentry: &mut DEntry<BS2Ramfs>,
        mode: u16,
    ) -> Result {
        pr_emerg!("enter mkdir");
        if let Err(_) = self.mknod(mnt_userns, dir, dentry, mode | bindings::S_IFDIR as _, 0) {
            pr_emerg!("mkdir: inc_nlink");
            dir.inc_nlink();
        }
        Ok(())
    }

    fn mknod(
        &self,
        _mnt_userns: &mut IdMap,
        dir: &mut INode<BS2Ramfs>,
        dentry: &mut DEntry<BS2Ramfs>,
        mode: u16,
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
}

pub fn ramfs_get_inode<'a>(
    sb: &'a mut SuperBlock<BS2Ramfs>,
    dir: Option<&'_ mut INode<BS2Ramfs>>,
    mode: u16,
    dev: bindings::dev_t,
) -> Option<&'a mut INode<BS2Ramfs>> {
    INode::new(sb).map(|inode| {
        inode.i_ino = INode::<BS2Ramfs>::next_ino() as _;
        inode.init_owner(unsafe { &mut bindings::init_user_ns }, dir, mode);

        inode.set_address_space_operations(unsafe {
            AddressSpaceOperationsVtable::<Bs2RamfsAOps>::build()
        }); // TODO: this should be done inside set_address_space_operations

        // I think these should be functions on the AddressSpace, i.e. sth like inode.get_address_space().set_gfp_mask(...)
        unsafe {
            rust_helper_mapping_set_gfp_mask(inode.i_mapping, RUST_HELPER_GFP_HIGHUSER);
            rust_helper_mapping_set_unevictable(inode.i_mapping);
        }

        inode.update_acm_time(UpdateATime::Yes, UpdateCTime::Yes, UpdateMTime::Yes);
        match mode as _ & bindings::S_IFMT {
            bindings::S_IFREG => {
                static I_OPS: bindings::inode_operations =
                    unsafe { *INodeOperationsVtable::<BS2Ramfs, BS2RamfsFileINodeOps>::build() }; // TODO: this should be done inside set_inode_operations
                inode.set_inode_operations(&I_OPS);
                inode.set_file_operations::<Bs2RamfsFileOps>();
            }
            bindings::S_IFDIR => {
                static I_OPS: BS2RamfsDirINodeOps = BS2RamfsDirINodeOps;
                inode.set_inode_operations(&I_OPS);
                inode.set_file_operations::<SimpleDirOperations>();
                inode.inc_nlink();
            }
            bindings::S_IFLNK => {
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
