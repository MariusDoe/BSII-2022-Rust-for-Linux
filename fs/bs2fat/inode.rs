use kernel::{
    bindings,
    file::TimeFlags,
    fs::{
        block_device::BlockDevice,
        inode::{Inode, WriteSync},
        libfs_functions,
        super_block::SuperBlock,
    },
    print::ExpectK,
    Error, Result,
};

use crate::{super_ops::msdos_sb, time::fat_truncate_time};

pub(crate) const FAT_ROOT_INO: u64 = 1;
pub(crate) const FAT_FSINFO_INO: u64 = 2;

pub(crate) fn fat_add_cluster(_inode: &mut Inode) -> Result {
    // int err, cluster;

    // err = fat_alloc_clusters(inode, &cluster, 1);
    // if (err)
    //     return err;
    // /* FIXME: this cluster should be added after data of this
    //  * cluster is writed */
    // err = fat_chain_add(inode, cluster, 1);
    // if (err)
    //     fat_free_clusters(inode, cluster);
    // return err;
    unimplemented!()
}

pub(crate) fn fat_cont_expand(inode: &mut Inode, size: bindings::loff_t) -> Result {
    libfs_functions::generic_cont_expand_simple(inode, size)?;
    fat_truncate_time(
        inode,
        None,
        TimeFlags::empty().with(TimeFlags::C).with(TimeFlags::M),
    );
    inode.mark_dirty();

    if !inode.is_sync() {
        return Ok(());
    }

    // niklas: This is odd, they only use count as start + count - 1 which is just size - 1
    let start = inode.i_size;
    let count = size - inode.i_size;
    let mapping = inode.i_mapping;

    // Opencode syncing since we don't have a file open to use standard fsync path.
    libfs_functions::filemap_fdatawrite_range(mapping, start, start + count - 1)
        .and(libfs_functions::sync_mapping_buffers(mapping))
        .and(inode.write_now(WriteSync::Yes))
        .and_then(|()| libfs_functions::filemap_fdatawait_range(mapping, start, start + count - 1))
}

pub(crate) fn fat_flush_inodes(
    sb: &mut SuperBlock,
    i1: Option<&mut Inode>,
    i2: Option<&mut Inode>,
) -> Result {
    if msdos_sb(sb).options.flush() != 0 {
        return Ok(());
    }
    writeback_inode(i1.ok_or(Error::EINVAL)?)?;
    writeback_inode(i2.ok_or(Error::EINVAL)?)?;
    libfs_functions::filemap_flush(unsafe {
        (*(sb.s_bdev as *mut BlockDevice))
            .bd_inode
            .as_mut()
            .expectk("bd_inode in block_device is null")
            .as_mut()
            .mapping()
    })
}

pub(crate) fn writeback_inode(inode: &mut Inode) -> Result {
    /* if we used wait=1, sync_inode_metadata waits for the io for the
     * inode to finish.  So wait=0 is sent down to sync_inode_metadata
     * and filemap_fdatawrite is used for the data blocks
     */
    libfs_functions::sync_inode_metadata(inode, 0)?;
    libfs_functions::filemap_fdatawrite(inode.mapping())
}
