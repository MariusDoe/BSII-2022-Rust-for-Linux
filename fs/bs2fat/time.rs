use kernel::{bindings, file::TimeFlags, fs::inode::Inode};

use crate::{
    inode::FAT_ROOT_INO,
    super_ops::{BS2FatSuperInfo, BS2FatSuperOps},
};

// DOS dates from 1980/1/1 through 2107/12/31
pub(crate) const FAT_DATE_MIN: u16 = 0 << 9 | 1 << 5 | 1;
pub(crate) const FAT_DATE_MAX: u16 = 127 << 9 | 12 << 5 | 31;
pub(crate) const FAT_TIME_MAX: u16 = 23 << 11 | 59 << 5 | 29;

pub(crate) const SECS_PER_MIN: i64 = 60;
pub(crate) const SECS_PER_HOUR: i64 = 60 * 60;
pub(crate) const SECS_PER_DAY: i64 = 60 * 60 * 24;

/// days between 1.1.70 and 1.1.80 (2 leap days)
const DAYS_DELTA: i64 = 365 * 10 + 2;
#[rustfmt::skip]
const DAYS_IN_YEAR: &[i64] = &[
    // Jan  Feb  Mar  Apr  May  Jun  Jul  Aug  Sep  Oct  Nov  Dec
    0,   0,  31,  59,  90, 120, 151, 181, 212, 243, 273, 304, 334, 0, 0, 0,
];
const YEAR_2100: i64 = 120;
const fn is_leap_year(year: i64) -> bool {
    (year & 0b11) == 0 && year != YEAR_2100
}

/// IMPORTANT: in contrast to the C signature, this function expects the given parameters to be in
/// the CPUs native representation and _does not_ convert them from little endian. As a caller, you
/// have to do this yourself
pub(crate) fn fat_time_to_unix_time(
    sbi: &BS2FatSuperInfo,
    time: u16,
    date: u16,
    time_cs: u8,
) -> bindings::timespec64 {
    let year = (date >> 9) as i64;
    let month = ((date >> 5) & 0xf).max(1) as usize;
    let day = ((date & 0x1f).max(1) - 1) as i64;
    let mut leap_day = (year + 3) / 4;
    if year > YEAR_2100 {
        leap_day -= 1;
    }
    if is_leap_year(year) && month > 2 {
        leap_day += 1;
    }

    let time = time as i64;
    let mut second = (time & 0x1f) << 1;
    second += ((time >> 5) & 0x3f) * SECS_PER_MIN;
    second += (time >> 11) * SECS_PER_HOUR;
    second += (year * 365 + leap_day + DAYS_IN_YEAR[month] + day + DAYS_DELTA) * SECS_PER_DAY;
    second += sbi.timezone_offset();

    if time_cs != 0 {
        let time_cs = time_cs as i64;
        bindings::timespec64 {
            tv_sec: second + (time_cs / 100),
            tv_nsec: (time_cs % 100) * 10_000_000,
        }
    } else {
        bindings::timespec64 {
            tv_sec: second,
            tv_nsec: 0,
        }
    }
}

/// truncate the various times with appropriate granularity:
///   root inode:
///     all times always 0
///   all other inodes:
///     mtime - 2 seconds
///     ctime
///       msdos - 2 seconds
///       vfat  - 10 milliseconds // niklas: we don't care
///     atime - 24 hours (00:00:00 in local timezone)
pub(crate) fn fat_truncate_time(
    inode: &mut Inode,
    now: Option<bindings::timespec64>,
    flags: TimeFlags,
) {
    if inode.i_ino == FAT_ROOT_INO {
        return;
    }

    // niklas: I changed the signature to take `now` by value, because we only read from it anyways
    let now = now.unwrap_or_else(|| inode.current_time());

    if flags.has(TimeFlags::A) {
        let sb_info: &BS2FatSuperOps = todo!(); // see allocate file
        let tz_offset = sb_info.timezone_offset();
        let seconds = now.tv_sec - tz_offset;
        let seconds = seconds + tz_offset - (seconds % SECS_PER_DAY);
        inode.i_atime = bindings::timespec64 {
            tv_sec: seconds,
            tv_nsec: 0,
        };
    }
    if flags.has(TimeFlags::C) {
        // niklas: I didn't bother to add the check for vfat
        inode.i_ctime = fat_timespec64_trunc_2secs(now);
    }
    if flags.has(TimeFlags::M) {
        inode.i_mtime = fat_timespec64_trunc_2secs(now);
    }
}

fn fat_timespec64_trunc_2secs(ts: bindings::timespec64) -> bindings::timespec64 {
    bindings::timespec64 {
        tv_sec: ts.tv_sec & !0b1,
        tv_nsec: 0,
    }
}
