#![allow(unused_variables)]
use core::ops::DerefMut;

use kernel::{
    bindings,
    c_types::*,
    file::{AllocMode, FMode, File, IoctlCommand, Operations, SeekFrom},
    fs::{inode::Inode, kiocb::Kiocb, libfs_functions},
    iov_iter::IovIter,
    mm::virt::Area,
    prelude::*,
    Error, Result, print::ExpectK,
};

use crate::{
    inode::{fat_add_cluster, fat_cont_expand, fat_flush_inodes},
    super_ops::{msdos_sb, BS2FatSuperOps},
};

extern "C" {
    static RUST_HELPER_HZ: c_long;
    fn rust_helper_congestion_wait(sync: c_int, timeout: c_long) -> c_long;
}

pub(crate) struct BS2FatFileOps;

impl Operations for BS2FatFileOps {
    kernel::declare_file_operations!(
        // release, // always used
        read_iter,
        write_iter,
        seek,
        ioctl,
        compat_ioctl,
        fsync,
        mmap,
        splice_read,
        splice_write,
        allocate_file
    );

    fn open(context: &(), file: &File) -> Result<Self::Data> {
        unimplemented!()
    }

    fn release(_data: Self::Data, file: &File) {
        // Assumption: Inode stems from file (! please verify); TODO:
        let inode: &mut Inode = file.inode();
        if file.fmode().has(FMode::FMODE_WRITE) && msdos_sb(inode.super_block_mut()).options.flush() != 0 {
            fat_flush_inodes(inode.super_block_mut(), Some(inode), None).expectk("error flush inodes");
            unsafe { bindings::io_schedule_timeout(RUST_HELPER_HZ / 10) };
        }
    }

    fn read_iter(_data: (), iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
        libfs_functions::generic_file_read_iter(iocb, iter)
    }

    fn write_iter(_data: (), iocb: &mut Kiocb, iter: &mut IovIter) -> Result<usize> {
        libfs_functions::generic_file_write_iter(iocb, iter)
    }

    fn seek(_data: (), file: &File, offset: SeekFrom) -> Result<u64> {
        libfs_functions::generic_file_llseek(file, offset)
    }

    fn ioctl(_data: (), _file: &File, _cmd: &mut IoctlCommand) -> Result<i32> {
        unimplemented!()
    }

    fn compat_ioctl(_data: (), file: &File, cmd: &mut IoctlCommand) -> Result<i32> {
        libfs_functions::compat_ptr_ioctl(file, cmd)
    }

    fn fsync(_data: (), file: &File, start: u64, end: u64, datasync: bool) -> Result<u32> {
        // let inode: inode = file.f_mapping.host;
        // int err;

        // libfs_functions::generic_file_fsync(filp, start, end, datasync);

        // err = sync_mapping_buffers(MSDOS_SB(inode->i_sb)->fat_inode->i_mapping);
        // if (err)
        //     return err;

        // return blkdev_issue_flush(inode->i_sb->s_bdev);
        unimplemented!()
    }

    fn mmap(_data: (), file: &File, vma: &mut Area) -> Result {
        libfs_functions::generic_file_mmap(file, vma)
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

    fn allocate_file(
        _data: (),
        file: &File,
        mode: AllocMode,
        offset: bindings::loff_t,
        length: bindings::loff_t,
    ) -> Result {
        if !mode.without(AllocMode::KEEP_SIZE).is_empty() {
            // No support for hole punch or other fallocate flags.
            return Err(Error::EOPNOTSUPP);
        }

        let inode = file.host_inode();

        if !inode.mode().is_regular_file() {
            return Err(Error::EOPNOTSUPP);
        }

        let end_offset = offset + length;
        let sb_info: &BS2FatSuperOps = todo!(); // inode.super_block().super_ops().as_mut();
        let inode = inode.lock();
        if mode.has(AllocMode::KEEP_SIZE) {
            pr_emerg!("since fat_add_cluster is not implemented, this isn't gonna end well");
            let size_on_disk = inode.i_blocks << 9;
            if end_offset <= size_on_disk as _ {
                return Ok(());
            }

            let bytes_for_file = end_offset as u64 - size_on_disk;
            let num_clusters =
                (bytes_for_file + sb_info.cluster_size as u64 - 1) >> sb_info.cluster_bits;

            for _ in 0..num_clusters {
                // Notably, these are not zeroed
                fat_add_cluster(inode.deref_mut())?;
            }

            Ok(())
        } else {
            if end_offset <= inode.size_read() {
                return Ok(());
            }

            // This is just an expanding truncate
            fat_cont_expand(inode.deref_mut(), end_offset)
        }
    }
}
