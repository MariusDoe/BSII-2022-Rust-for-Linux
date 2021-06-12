use core::ptr;

use crate::{
    bindings,
    c_types::*,
    error::Error,
    file::File,
    file_operations::SeekFrom,
    fs::{
        dentry::Dentry, from_kernel_err_ptr, inode::Inode, kiocb::Kiocb, super_block::SuperBlock,
        super_operations::Kstatfs, DeclaredFileSystemType, FileSystemBase,
    },
    iov_iter::IovIter,
    print::ExpectK,
    str::CStr,
    types::{Iattr, Kstat, Path, UserNamespace},
    Result,
};

pub fn generic_file_read_iter(iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
    Error::parse_int(unsafe { bindings::generic_file_read_iter(iocb.as_ptr_mut(), iter.ptr) as _ })
}

pub fn generic_file_write_iter(iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
    Error::parse_int(unsafe { bindings::generic_file_write_iter(iocb.as_ptr_mut(), iter.ptr) as _ })
}

pub fn generic_file_mmap(file: &File, vma: &mut bindings::vm_area_struct) -> Result {
    Error::parse_int(unsafe { bindings::generic_file_mmap(file.ptr, vma as *mut _) }).map(|_| ())
}

pub fn noop_fsync(file: &File, start: u64, end: u64, datasync: bool) -> Result<u32> {
    let start = start as _;
    let end = end as _;
    let datasync = if datasync { 1 } else { 0 };
    let res = unsafe { bindings::noop_fsync(file.ptr, start, end, datasync) };
    if res == 0 {
        Ok(0)
    } else {
        Err(Error::EINVAL)
        // Err(Error::from_kernel_errno(bindings::errno))
    }
}

pub fn generic_file_llseek(file: &File, pos: SeekFrom) -> Result<u64> {
    let (offset, whence) = pos.into_pos_and_whence();
    Error::parse_int(
        unsafe { bindings::generic_file_llseek(file.ptr, offset as _, whence as _) } as _,
    )
}

pub fn generic_file_splice_read(
    file: &File,
    pos: *mut i64,
    pipe: &mut bindings::pipe_inode_info,
    len: usize,
    flags: u32,
) -> Result<usize> {
    Error::parse_int(unsafe {
        bindings::generic_file_splice_read(file.ptr, pos, pipe as *mut _, len, flags) as _
    })
}

pub fn iter_file_splice_write(
    pipe: &mut bindings::pipe_inode_info,
    file: &File,
    pos: *mut i64,
    len: usize,
    flags: u32,
) -> Result<usize> {
    Error::parse_int(unsafe {
        bindings::iter_file_splice_write(pipe as *mut _, file.ptr, pos, len, flags) as _
    })
}

pub fn generic_delete_inode(inode: &mut Inode) -> Result {
    Error::parse_int(unsafe { bindings::generic_delete_inode(inode.as_ptr_mut()) }).map(|_| ())
}

pub fn simple_statfs(root: &mut Dentry, buf: &mut Kstatfs) -> Result {
    Error::parse_int(unsafe { bindings::simple_statfs(root.as_ptr_mut(), buf as *mut _) })
        .map(|_| ())
}

pub fn simple_setattr(
    mnt_userns: &mut UserNamespace,
    dentry: &mut Dentry,
    iattr: &mut Iattr,
) -> Result {
    Error::parse_int(unsafe {
        bindings::simple_setattr(mnt_userns as *mut _, dentry.as_ptr_mut(), iattr as *mut _)
    })
    .map(|_| ())
}

pub fn simple_getattr(
    mnt_userns: &mut UserNamespace,
    path: &Path,
    stat: &mut Kstat,
    request_mask: u32,
    query_flags: u32,
) -> Result {
    Error::parse_int(unsafe {
        bindings::simple_getattr(
            mnt_userns as *mut _,
            path as *const _,
            stat as *mut _,
            request_mask,
            query_flags,
        )
    })
    .map(|_| ())
}

pub fn simple_lookup(dir: &mut Inode, dentry: &mut Dentry, flags: c_uint) -> Result<*mut Dentry> {
    // todo: return type ptr vs ref?
    from_kernel_err_ptr(unsafe {
        bindings::simple_lookup(dir.as_ptr_mut(), dentry.as_ptr_mut(), flags) as *mut _
    })
}

pub fn simple_link(old_dentry: &mut Dentry, dir: &mut Inode, dentry: &mut Dentry) -> Result {
    Error::parse_int(unsafe {
        bindings::simple_link(
            old_dentry.as_ptr_mut(),
            dir.as_ptr_mut(),
            dentry.as_ptr_mut(),
        )
    })
    .map(|_| ())
}

pub fn simple_unlink(dir: &mut Inode, dentry: &mut Dentry) -> Result {
    Error::parse_int(unsafe { bindings::simple_unlink(dir.as_ptr_mut(), dentry.as_ptr_mut()) })
        .map(|_| ())
}

pub fn simple_rmdir(dir: &mut Inode, dentry: &mut Dentry) -> Result {
    Error::parse_int(unsafe { bindings::simple_rmdir(dir.as_ptr_mut(), dentry.as_ptr_mut()) })
        .map(|_| ())
}

pub fn simple_rename(
    mnt_userns: &mut UserNamespace,
    old_dir: &mut Inode,
    old_dentry: &mut Dentry,
    new_dir: &mut Inode,
    new_dentry: &mut Dentry,
    flags: c_uint,
) -> Result {
    Error::parse_int(unsafe {
        bindings::simple_rename(
            mnt_userns as *mut _,
            old_dir.as_ptr_mut(),
            old_dentry.as_ptr_mut(),
            new_dir.as_ptr_mut(),
            new_dentry.as_ptr_mut(),
            flags,
        )
    })
    .map(|_| ())
}

pub fn page_symlink(inode: &mut Inode, symname: &'static CStr) -> Result {
    Error::parse_int(unsafe {
        bindings::page_symlink(
            inode.as_ptr_mut(),
            symname.as_ptr() as _,
            symname.len_with_nul() as _,
        )
    })
    .map(|_| ())
}

pub fn register_filesystem<T: DeclaredFileSystemType>() -> Result {
    Error::parse_int(unsafe { bindings::register_filesystem(T::file_system_type()) }).map(|_| ())
}

pub fn unregister_filesystem<T: DeclaredFileSystemType>() -> Result {
    Error::parse_int(unsafe { bindings::unregister_filesystem(T::file_system_type()) }).map(|_| ())
}

pub fn mount_nodev<T: DeclaredFileSystemType>(
    flags: c_int,
    data: Option<&mut T::MountOptions>,
) -> Result<*mut bindings::dentry> {
    from_kernel_err_ptr(unsafe {
        bindings::mount_nodev(
            T::file_system_type(),
            flags,
            data.map(|p| p as *mut _ as *mut _)
                .unwrap_or_else(ptr::null_mut),
            Some(fill_super_callback::<T>),
        )
    })
}

unsafe extern "C" fn fill_super_callback<T: FileSystemBase>(
    sb: *mut bindings::super_block,
    data: *mut c_void,
    silent: c_int,
) -> c_int {
    unsafe {
        let sb = sb.as_mut().expectk("SuperBlock was null").as_mut();
        let data = (data as *mut T::MountOptions).as_mut();
        T::fill_super(sb, data, silent)
            .map(|_| 0)
            .unwrap_or_else(|e| e.to_kernel_errno())
    }
}

pub fn kill_litter_super(sb: &mut SuperBlock) {
    unsafe {
        bindings::kill_litter_super(sb.as_ptr_mut());
    }
}
