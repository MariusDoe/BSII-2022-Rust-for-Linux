use alloc::boxed::Box;
use core::{cmp::Ord, mem, ptr};

use kernel::{
    bindings,
    c_types::*,
    fs::{
        dentry::Dentry, inode::Inode, libfs_functions, super_block::SuperBlock, FileSystemBase,
        FileSystemType,
    },
    prelude::*,
    str::CStr,
    Error, Module,
};

mod bootsector;
mod file;
mod inode;
mod super_ops;
mod time;

use bootsector::{fat_read_bpb, BootSector};
use super_ops::BS2FatSuperOps;
use time::{fat_time_to_unix_time, FAT_DATE_MAX, FAT_DATE_MIN, FAT_TIME_MAX};

use crate::{
    inode::{FAT_FSINFO_INO, FAT_ROOT_INO},
    super_ops::BS2FatSuperInfo,
};

module! {
    type: BS2Fat,
    name: b"bs2fat",
    author: b"Rust for Linux Contributors",
    description: b"MS-DOS filesystem support",
    license: b"GPL v2",
}

const FAT_NAME_LENGTH: usize = 11;
// Characters that are undesirable in an MS-DOS file name
const BAD_CHARS: &[u8] = b"*?<>|\"";
const BAD_IF_STRICT: &[u8] = b"+=,; ";

/// start of data cluster's entry (number of reserved clusters)
const FAT_START_ENT: u32 = 2;
const MSDOS_SUPER_MAGIC: u64 = 0x4d44;

const FAT_STATE_DIRTY: u8 = 1;

const FAT12_MAX_CLUSTERS: usize = 0xff4;
const FAT16_MAX_CLUSTERS: usize = 0xfff4;

struct BS2Fat;

type Cluster = u32;

enum FillSuperErrorKind {
    Invalid,
    Fail(Error),
}

impl FileSystemBase for BS2Fat {
    const NAME: &'static CStr = kernel::c_str!("bs2fat");
    const FS_FLAGS: c_int = (bindings::FS_REQUIRES_DEV | bindings::FS_ALLOW_IDMAP) as _;
    const OWNER: *mut bindings::module = ptr::null_mut();

    fn mount(
        _fs_type: &'_ mut FileSystemType,
        flags: c_int,
        device_name: &CStr,
        data: Option<&mut Self::MountOptions>,
    ) -> Result<*mut bindings::dentry> {
        libfs_functions::mount_bdev::<Self>(flags, device_name, data)
    }

    fn kill_super(sb: &mut SuperBlock) {
        libfs_functions::kill_block_super(sb);
    }

    fn fill_super(
        mut sb: &mut SuperBlock,
        data: Option<&mut Self::MountOptions>,
        silent: c_int,
    ) -> Result {
        let silent = silent == 1; // FIXME: why do we not do this in the lib callback?

        let res = init_superblock_and_info(&mut sb, silent);

        res.map_err(|err| {
            use FillSuperErrorKind::*;
            let error_val = match err {
                Invalid => {
                    if !silent {
                        // TODO: what is fat_msg? sb is given to it too ...
                        pr_info!("Can't find a valid FAT filesystem");
                    }
                    Error::EINVAL
                }
                Fail(e) => e,
            };

            // TODO some nls things

            if let Some(ops) = sb.take_super_operations::<BS2FatSuperOps>() {
                // SAFETY: ops was written with Box::leak before
                let mut ops: Box<BS2FatSuperOps> = unsafe { Box::from_raw(ops as *const _ as _) };
                unsafe {
                    if let Some(inode_ptr) = ops.fsinfo_inode.take() {
                        (*inode_ptr).put();
                    }
                    if let Some(inode_ptr) = ops.fat_inode.take() {
                        (*inode_ptr).put();
                    }
                }
                drop(ops);
            }

            error_val
        })
    }
}

fn init_superblock_and_info(
    sb: &mut SuperBlock,
    silent: bool,
) -> core::result::Result<(), FillSuperErrorKind> {
    use FillSuperErrorKind::*;

    let mut info = BS2FatSuperInfo::default();

    sb.s_flags |= bindings::SB_NODIRATIME as u64;
    sb.s_magic = MSDOS_SUPER_MAGIC;
    // sb.s_export_op = &fat_export_ops; // FIXME
    sb.s_time_gran = 1;
    // sbi.nfs_build_inode_lock = Mutex::ratelimit_state_init(ops.ratelimit, DEFAULT_RATELIMIT_INTERVAL, DEFAULT_RATELIMIT_BURST); // FIXME
    // parse_options(
    //     sb,
    //     data,
    //     false, /* is_vfat */
    //     silent,
    //     &debug,
    //     ops.options,
    // )
    // .map_err(Fail)?;
    // niklas: C calls the given "setup" here, I inlined that
    // niklas, later: let's first see how this is used, maybe we can make it rust-y and
    // have a field of ops be a &(dyn InodeOperations) or so
    // MSDOS_SB(sb)->dir_ops = &msdos_dir_inode_operations; // TODO This should be done in BS2FatSuperOps::default()
    // sb.set_dentry_operations::<BS2FatDentryOps>();
    sb.s_flags |= bindings::SB_NOATIME as u64;
    sb.set_min_blocksize(512);
    let buffer_head = sb.read_block(0).ok_or_else(|| {
        pr_err!("unable to read boot sector");
        Fail(Error::EIO)
    })?;
    let boot_sector = unsafe { buffer_head.b_data.cast::<BootSector>().read_unaligned() };
    let bpb = fat_read_bpb(sb, boot_sector, silent);
    libfs_functions::release_buffer(buffer_head);
    let bpb = bpb.map_err(|e| if e == Error::EINVAL { Invalid } else { Fail(e) })?;
    let logical_sector_size = bpb.sector_size as u64;
    // FIXME see comment above
    info.sectors_per_cluster = bpb.sectors_per_cluster as _;
    if logical_sector_size < sb.s_blocksize {
        pr_err!(
            "logical sector size too small for device ({})",
            logical_sector_size
        );
        return Err(Fail(Error::EIO));
    }
    if logical_sector_size > sb.s_blocksize {
        if sb.set_blocksize(logical_sector_size as _) != 0 {
            pr_err!("unable to set blocksize {}", logical_sector_size);
            return Err(Fail(Error::EIO));
        }

        if let Some(bh_resize) = sb.read_block(0) {
            libfs_functions::release_buffer(bh_resize);
        } else {
            pr_err!(
                "unable to read boot sector (logical sector size {})",
                sb.s_blocksize
            );
            return Err(Fail(Error::EIO));
        }
    }
    // mutex_init => TODO should be done in constructor / default
    info.cluster_size = sb.s_blocksize as u32 * info.sectors_per_cluster as u32;
    info.cluster_bits = info.cluster_size.trailing_zeros() as _;
    // TODO someone sanity-check please
    info.fats = bpb.fats;
    info.fat_bits = 0;
    // don't know yet
    info.fat_start = bpb.reserved;
    info.fat_length = bpb.fat_length;
    info.root_cluster = 0;
    info.free_clusters = u32::MAX;
    // don't know yet
    info.free_clusters_valid = 0;
    info.previous_free = FAT_START_ENT;
    sb.s_maxbytes = 0xffffffff;
    sb.s_time_min = fat_time_to_unix_time(&info, 0, FAT_DATE_MIN, 0).tv_sec;
    sb.s_time_max = fat_time_to_unix_time(&info, FAT_TIME_MAX, FAT_DATE_MAX, 0).tv_sec;
    // skipping over the
    //     if (!sbi->fat_length && bpb.fat32_length) { ... }
    info.volume_id = bpb.fat16_vol_id;
    info.dir_per_block = (sb.s_blocksize / mem::size_of::<Bs2FatDirEntry>() as u64) as _;
    info.dir_per_block_bits = info.dir_per_block.trailing_zeros() as _;
    // TODO someone sanity check please
    info.dir_start = info.fat_start as usize + info.fats as usize * info.fat_length as usize;
    info.dir_entries = bpb.dir_entries;
    if info.dir_entries as i32 & (info.dir_per_block - 1) != 0 {
        if !silent {
            pr_err!("bogus number of directory entries ({})", info.dir_entries);
        }
        return Err(Invalid);
    }
    let rootdir_sectors =
        info.dir_entries as usize * mem::size_of::<Bs2FatDirEntry>() / sb.s_blocksize as usize;
    info.data_start = info.dir_start + rootdir_sectors;
    let total_sectors = Some(bpb.sectors)
        .filter(|&x| x != 0)
        .unwrap_or(bpb.total_sectors as _);
    let total_clusters =
        (total_sectors as usize - info.data_start) / info.sectors_per_cluster as usize;
    info.fat_bits = match total_clusters {
        x if x <= FAT12_MAX_CLUSTERS => 12,
        _ => 16,
    };
    info.dirty = (bpb.fat16_state & FAT_STATE_DIRTY) as _;
    // FIXME wrapper
    // check that the table doesn't overflow
    let fat_clusters = calc_fat_clusters(&info, &sb);
    let total_clusters = total_clusters.min(fat_clusters - FAT_START_ENT as usize);
    if total_clusters > info.max_fats() {
        if !silent {
            pr_err!("count of clusters too big ({})", total_clusters);
        }
        return Err(Invalid);
    }
    info.max_cluster = total_clusters + FAT_START_ENT as usize;
    if info.free_clusters > total_clusters as u32 {
        info.free_clusters = u32::MAX;
    }
    info.previous_free = (info.previous_free % info.max_cluster as u32).max(FAT_START_ENT);
    // set up enough so that it can read an inode
    // FIXME currently, we haven't set the super ops yet, becaues we are still editing the
    // struct
    fat_hash_init(sb);
    dir_hash_init(sb);
    fat_ent_access_init(sb);
    // TODO something about nls and codepages, let's first check whether that is important
    info.fat_inode = Some(Inode::new(sb).ok_or(Fail(Error::ENOMEM))?);
    info.fsinfo_inode = {
        let inode = Inode::new(sb).ok_or(Fail(Error::ENOMEM))?;
        inode.i_ino = FAT_FSINFO_INO;
        inode.insert_hash();
        Some(inode)
    };
    sb.s_root = {
        let inode = Inode::new(sb).ok_or(Fail(Error::ENOMEM))?;
        inode.i_ino = FAT_ROOT_INO;
        inode.set_iversion(1);
        if let Err(e) = fat_read_root(inode, &info) {
            inode.put();
            return Err(Fail(e));
        }
        inode.insert_hash();
        fat_attach(inode, 0);
        Dentry::make_root(inode)
            .ok_or_else(|| {
                pr_err!("get root inode failed");
                Fail(Error::ENOMEM)
            })?
            .as_ptr_mut()
    };
    // TODO something about the "discard" option
    fat_set_state(sb, 1, 0);

    // SAFETY: TODO
    let ops = unsafe { BS2FatSuperOps::new_from_info(info) };
    let pointer = Box::leak(Box::try_new(ops).map_err(|_| Fail(Error::ENOMEM))?);
    sb.set_super_operations(pointer);

    Ok(())
}

kernel::declare_fs_type!(BS2Fat, BS2FAT_FS_TYPE);

impl Module for BS2Fat {
    fn init(_name: &'static CStr, _module: &'static ThisModule) -> Result<Self> {
        pr_emerg!("bs2 fat in action");
        libfs_functions::register_filesystem::<Self>().map(move |_| Self)
    }
}

impl Drop for BS2Fat {
    fn drop(&mut self) {
        let _ = libfs_functions::unregister_filesystem::<Self>();
        pr_info!("bs2 fat out of action");
    }
}

fn fat_hash_init(sb: &mut SuperBlock) {
    unimplemented!()
}
fn dir_hash_init(sb: &mut SuperBlock) {
    unimplemented!()
}
fn fat_ent_access_init(sb: &mut SuperBlock) {
    unimplemented!()
}

fn calc_fat_clusters(info: &BS2FatSuperInfo, sb: &SuperBlock) -> usize {
    const BITS_PER_BYTE: usize = 8;
    info.fat_length as usize * sb.s_blocksize as usize * BITS_PER_BYTE / info.fat_bits as usize
}

fn fat_read_root(root_inode: &mut Inode, info: &BS2FatSuperInfo) -> Result {
    /*
    // C allocates msdos_inode_info around each inode and accesses more data this way.
    // We don't know yet why. Maybe to save one alloc call for the intended i_private pointer
    // that is supposed to hold this kind of info.
    //
    // C registers alloc_inode for new_inode
    MSDOS_I(inode)->i_pos = MSDOS_ROOT_INO;
    inode->i_uid = sbi->options.fs_uid;
    inode->i_gid = sbi->options.fs_gid;
    inode_inc_iversion(inode);
    inode->i_generation = 0;
    inode->i_mode = fat_make_mode(sbi, ATTR_DIR, S_IRWXUGO);
    inode->i_op = sbi->dir_ops;
    inode->i_fop = &fat_dir_operations;
    if (is_fat32(sbi)) {
        MSDOS_I(inode)->i_start = sbi->root_cluster;
        error = fat_calc_dir_size(inode);
        if (error < 0)
            return error;
    } else {
        MSDOS_I(inode)->i_start = 0;
        inode->i_size = sbi->dir_entries * sizeof(struct msdos_dir_entry);
    }
    inode->i_blocks = ((inode->i_size + (sbi->cluster_size - 1))
                & ~((loff_t)sbi->cluster_size - 1)) >> 9;
    MSDOS_I(inode)->i_logstart = 0;
    MSDOS_I(inode)->mmu_private = inode->i_size;

    fat_save_attrs(inode, ATTR_DIR);
    inode->i_mtime.tv_sec = inode->i_atime.tv_sec = inode->i_ctime.tv_sec = 0;
    inode->i_mtime.tv_nsec = inode->i_atime.tv_nsec = inode->i_ctime.tv_nsec = 0;
    set_nlink(inode, fat_subdirs(inode)+2);
    */
    unimplemented!()
}

fn fat_calc_dir_size(inode: &mut Inode) -> Result {
    /*
    struct msdos_sb_info *sbi = MSDOS_SB(inode->i_sb);
    int ret, fclus, dclus;

    inode->i_size = 0;
    if MSDOS_I(inode)->i_start == 0 {
        return Ok(());
    }

    ret = fat_get_cluster(inode, FAT_ENT_EOF, &fclus, &dclus);
    if (ret < 0)
        return ret;
    inode->i_size = (fclus + 1) << sbi->cluster_bits;
    */
    unimplemented!()
}

fn fat_attach(root_inode: &mut Inode, some_number: usize) {
    unimplemented!()
}
fn fat_set_state(sb: &mut SuperBlock, anumber: usize, anothernumber: usize) {
    unimplemented!()
}

#[repr(C)]
struct Bs2FatDirEntry {
    name: [u8; FAT_NAME_LENGTH],
    /// ATTR_READ_ONLY:    0x01
    /// ATTR_HIDDEN:       0x02
    /// ATTR_SYSTEM:       0x04
    /// ATTR_VOLUME_ID:    0x08
    /// ATTR_DIRECTORY:    0x10
    /// ATTR_ARCHIVE:      0x20
    attributes: u8,
    reserved: u8,
    /// Hundredths of a second: 0-199
    creation_time_centiseconds: u8,
    /// Binary format: `hhhhhmmmmmmsssss`
    /// * hour (`h`): 0-23
    /// * minute (`m`): 0-59
    /// * second/2 (`s`): 0-29
    creation_time: u16,
    /// Binary format: `yyyyyyymmmmddddd`
    /// * year (`y`): 0-127 ~ 1980-2107
    /// * month (`m`): 1-12
    /// * day (`d`): 1-31
    creation_date: u16,
    /// Binary format: `yyyyyyymmmmddddd`
    /// * year (`y`): 0-127 ~ 1980-2107
    /// * month (`m`): 1-12
    /// * day (`d`): 1-31
    access_date: u16,
    cluster_number_high: u16,
    /// Binary format: `hhhhhmmmmmmsssss`
    /// * hour (`h`): 0-23
    /// * minute (`m`): 0-59
    /// * second/2 (`s`): 0-29
    modification_time: u16,
    /// Binary format: `yyyyyyymmmmddddd`
    /// * year (`y`): 0-127 ~ 1980-2107
    /// * month (`m`): 1-12
    /// * day (`d`): 1-31
    modification_date: u16,
    cluster_number_low: u16,
    /// File size in bytes
    size: u32,
}

#[derive(Default)]
pub struct BS2FatMountOptions {
    timezone_set: bool,
    time_offset: i64,
    flush: bool,
}

// fn fat_alloc_clusters(inode: &mut Inode, cluster: &mut Cluster, nr_cluster: u32)
// {
//     struct super_block *sb = inode->i_sb;
//     struct msdos_sb_info *sbi = MSDOS_SB(sb);
//     const struct fatent_operations *ops = sbi->fatent_ops;
//     struct fat_entry fatent, prev_ent;
//     struct buffer_head *bhs[MAX_BUF_PER_PAGE];
//     int i, count, err, nr_bhs, idx_clus;

//     BUG_ON(nr_cluster > (MAX_BUF_PER_PAGE / 2));    /* fixed limit */
//     lock_fat(sbi);
//     if (sbi->free_clusters != -1 && sbi->free_clus_valid &&
//         sbi->free_clusters < nr_cluster) {
//         unlock_fat(sbi);
//         return -ENOSPC;
//     }

//     err = nr_bhs = idx_clus = 0;
//     count = FAT_START_ENT;
//     fatent_init(&prev_ent);
//     fatent_init(&fatent);
//     fatent_set_entry(&fatent, sbi->prev_free + 1);
//     while (count < sbi->max_cluster) {
//         if (fatent.entry >= sbi->max_cluster)
//             fatent.entry = FAT_START_ENT;
//         fatent_set_entry(&fatent, fatent.entry);
//         err = fat_ent_read_block(sb, &fatent);
//         if (err)
//             goto out;

//         /* Find the free entries in a block */
//         do {
//             if (ops->ent_get(&fatent) == FAT_ENT_FREE) {
//                 int entry = fatent.entry;

//                 /* make the cluster chain */
//                 ops->ent_put(&fatent, FAT_ENT_EOF);
//                 if (prev_ent.nr_bhs)
//                     ops->ent_put(&prev_ent, entry);

//                 fat_collect_bhs(bhs, &nr_bhs, &fatent);

//                 sbi->prev_free = entry;
//                 if (sbi->free_clusters != -1)
//                     sbi->free_clusters--;

//                 cluster[idx_clus] = entry;
//                 idx_clus++;
//                 if (idx_clus == nr_cluster)
//                     goto out;

//                 /*
//                  * fat_collect_bhs() gets ref-count of bhs,
//                  * so we can still use the prev_ent.
//                  */
//                 prev_ent = fatent;
//             }
//             count++;
//             if (count == sbi->max_cluster)
//                 break;
//         } while (fat_ent_next(sbi, &fatent));
//     }

//     /* Couldn't allocate the free entries */
//     sbi->free_clusters = 0;
//     sbi->free_clus_valid = 1;
//     err = -ENOSPC;

// out:
//     unlock_fat(sbi);
//     mark_fsinfo_dirty(sb);
//     fatent_brelse(&fatent);
//     if (!err) {
//         if (inode_needs_sync(inode))
//             err = fat_sync_bhs(bhs, nr_bhs);
//         if (!err)
//             err = fat_mirror_bhs(sb, bhs, nr_bhs);
//     }
//     for (i = 0; i < nr_bhs; i++)
//         brelse(bhs[i]);

//     if (err && idx_clus)
//         fat_free_clusters(inode, cluster[0]);

//     return err;
// }

// int fat_free_clusters(struct inode *inode, int cluster)
// {
// 	struct super_block *sb = inode->i_sb;
// 	struct msdos_sb_info *sbi = MSDOS_SB(sb);
// 	const struct fatent_operations *ops = sbi->fatent_ops;
// 	struct fat_entry fatent;
// 	struct buffer_head *bhs[MAX_BUF_PER_PAGE];
// 	int i, err, nr_bhs;
// 	int first_cl = cluster, dirty_fsinfo = 0;

// 	nr_bhs = 0;
// 	fatent_init(&fatent);
// 	lock_fat(sbi);
// 	do {
// 		cluster = fat_ent_read(inode, &fatent, cluster);
// 		if (cluster < 0) {
// 			err = cluster;
// 			goto error;
// 		} else if (cluster == FAT_ENT_FREE) {
// 			fat_fs_error(sb, "%s: deleting FAT entry beyond EOF",
// 				     __func__);
// 			err = -EIO;
// 			goto error;
// 		}

// 		if (sbi->options.discard) {
// 			/*
// 			 * Issue discard for the sectors we no longer
// 			 * care about, batching contiguous clusters
// 			 * into one request
// 			 */
// 			if (cluster != fatent.entry + 1) {
// 				int nr_clus = fatent.entry - first_cl + 1;

// 				sb_issue_discard(sb,
// 					fat_clus_to_blknr(sbi, first_cl),
// 					nr_clus * sbi->sec_per_clus,
// 					GFP_NOFS, 0);

// 				first_cl = cluster;
// 			}
// 		}

// 		ops->ent_put(&fatent, FAT_ENT_FREE);
// 		if (sbi->free_clusters != -1) {
// 			sbi->free_clusters++;
// 			dirty_fsinfo = 1;
// 		}

// 		if (nr_bhs + fatent.nr_bhs > MAX_BUF_PER_PAGE) {
// 			if (sb->s_flags & SB_SYNCHRONOUS) {
// 				err = fat_sync_bhs(bhs, nr_bhs);
// 				if (err)
// 					goto error;
// 			}
// 			err = fat_mirror_bhs(sb, bhs, nr_bhs);
// 			if (err)
// 				goto error;
// 			for (i = 0; i < nr_bhs; i++)
// 				brelse(bhs[i]);
// 			nr_bhs = 0;
// 		}
// 		fat_collect_bhs(bhs, &nr_bhs, &fatent);
// 	} while (cluster != FAT_ENT_EOF);

// 	if (sb->s_flags & SB_SYNCHRONOUS) {
// 		err = fat_sync_bhs(bhs, nr_bhs);
// 		if (err)
// 			goto error;
// 	}
// 	err = fat_mirror_bhs(sb, bhs, nr_bhs);
// error:
// 	fatent_brelse(&fatent);
// 	for (i = 0; i < nr_bhs; i++)
// 		brelse(bhs[i]);
// 	unlock_fat(sbi);
// 	if (dirty_fsinfo)
// 		mark_fsinfo_dirty(sb);

// 	return err;
// }

// int fat_chain_add(struct inode *inode, int new_dclus, int nr_cluster)
// {
// 	struct super_block *sb = inode->i_sb;
// 	struct msdos_sb_info *sbi = MSDOS_SB(sb);
// 	int ret, new_fclus, last;

// 	/*
// 	 * We must locate the last cluster of the file to add this new
// 	 * one (new_dclus) to the end of the link list (the FAT).
// 	 */
// 	last = new_fclus = 0;
// 	if (MSDOS_I(inode)->i_start) {
// 		int fclus, dclus;

// 		ret = fat_get_cluster(inode, FAT_ENT_EOF, &fclus, &dclus);
// 		if (ret < 0)
// 			return ret;
// 		new_fclus = fclus + 1;
// 		last = dclus;
// 	}

// 	/* add new one to the last of the cluster chain */
// 	if (last) {
// 		struct fat_entry fatent;

// 		fatent_init(&fatent);
// 		ret = fat_ent_read(inode, &fatent, last);
// 		if (ret >= 0) {
// 			int wait = inode_needs_sync(inode);
// 			ret = fat_ent_write(inode, &fatent, new_dclus, wait);
// 			fatent_brelse(&fatent);
// 		}
// 		if (ret < 0)
// 			return ret;
// 		/*
// 		 * FIXME:Although we can add this cache, fat_cache_add() is
// 		 * assuming to be called after linear search with fat_cache_id.
// 		 */
// //		fat_cache_add(inode, new_fclus, new_dclus);
// 	} else {
// 		MSDOS_I(inode)->i_start = new_dclus;
// 		MSDOS_I(inode)->i_logstart = new_dclus;
// 		/*
// 		 * Since generic_write_sync() synchronizes regular files later,
// 		 * we sync here only directories.
// 		 */
// 		if (S_ISDIR(inode->i_mode) && IS_DIRSYNC(inode)) {
// 			ret = fat_sync_inode(inode);
// 			if (ret)
// 				return ret;
// 		} else
// 			mark_inode_dirty(inode);
// 	}
// 	if (new_fclus != (inode->i_blocks >> (sbi->cluster_bits - 9))) {
// 		fat_fs_error(sb, "clusters badly computed (%d != %llu)",
// 			     new_fclus,
// 			     (llu)(inode->i_blocks >> (sbi->cluster_bits - 9)));
// 		fat_cache_inval_inode(inode);
// 	}
// 	inode->i_blocks += nr_cluster << (sbi->cluster_bits - 9);

// 	return 0;
// }
