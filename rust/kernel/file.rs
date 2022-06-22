// SPDX-License-Identifier: GPL-2.0

//! Files and file descriptors.
//!
//! C headers: [`include/linux/fs.h`](../../../../include/linux/fs.h) and
//! [`include/linux/file.h`](../../../../include/linux/file.h)

use crate::{
    bindings, c_types,
    cred::Credential,
    error::{code::*, from_kernel_result, Error, Result},
    fs::{inode::Inode, kiocb::Kiocb, BuildVtable},
    io_buffer::{IoBufferReader, IoBufferWriter},
    iov_iter::IovIter,
    mm,
    print::ExpectK,
    sync::CondVar,
    types::PointerWrapper,
    user_ptr::{UserSlicePtr, UserSlicePtrReader, UserSlicePtrWriter},
    ARef, AlwaysRefCounted,
};
use core::convert::{TryFrom, TryInto};
use core::{cell::UnsafeCell, marker, mem, ptr};

/// Wraps the kernel's `struct file`.
///
/// # Invariants
///
/// Instances of this type are always ref-counted, that is, a call to `get_file` ensures that the
/// allocation remains valid at least until the matching call to `fput`.
#[repr(transparent)]
pub struct File(pub(crate) UnsafeCell<bindings::file>);

// TODO: Accessing fields of `struct file` through the pointer is UB because other threads may be
// writing to them. However, this is how the C code currently operates: naked reads and writes to
// fields. Even if we used relaxed atomics on the Rust side, we can't force this on the C side.
impl File {
    /// Constructs a new [`struct file`] wrapper from a file descriptor.
    ///
    /// The file descriptor belongs to the current process.
    pub fn from_fd(fd: u32) -> Result<ARef<Self>> {
        // SAFETY: FFI call, there are no requirements on `fd`.
        let ptr = ptr::NonNull::new(unsafe { bindings::fget(fd) }).ok_or(EBADF)?;

        // SAFETY: `fget` increments the refcount before returning.
        Ok(unsafe { ARef::from_raw(ptr.cast()) })
    }

    /// Creates a reference to a [`File`] from a valid pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is valid and remains valid for the lifetime of the
    /// returned [`File`] instance.
    pub(crate) unsafe fn from_ptr<'a>(ptr: *const bindings::file) -> &'a File {
        // SAFETY: The safety requirements guarantee the validity of the dereference, while the
        // `File` type being transparent makes the cast ok.
        unsafe { &*ptr.cast() }
    }

    /// Provides a *mut to the underlying file
    ///
    /// The pointer is properly aligned, dereferencable and points to an initialized instance
    pub(crate) fn as_mut_ptr(&self) -> *mut bindings::file {
        // Since `&self.0` is a valid reference, thus `self.0.get()` fulfills the above guarantees
        self.0.get()
    }

    /// Returns the current seek/cursor/pointer position (`struct file::f_pos`).
    pub fn pos(&self) -> u64 {
        // SAFETY: The file is valid because the shared reference guarantees a nonzero refcount.
        unsafe { core::ptr::addr_of!((*self.0.get()).f_pos).read() as _ }
    }

    /// Returns whether the file is in blocking mode.
    pub fn is_blocking(&self) -> bool {
        self.flags() & bindings::O_NONBLOCK == 0
    }

    pub fn inode(&self) -> &mut Inode {
        unsafe {
            self.as_mut_ptr()
                .f_inode
                .as_mut()
                .expectk("File had NULL inode")
                .as_mut()
        }
    }

    pub fn host_inode(&self) -> &mut Inode {
        unsafe {
            self.as_mut_ptr()
                .f_mapping
                .as_mut()
                .expectk("File had NULL mapping")
                .host
                .as_mut()
                .expectk("file mapping hat NULL host")
                .as_mut()
        }
    }

    pub fn fmode(&self) -> FMode {
        FMode::from_int(unsafe { self.as_mut_ptr().f_mode })
    }

    /// Returns the credentials of the task that originally opened the file.
    pub fn cred(&self) -> &Credential {
        // SAFETY: The file is valid because the shared reference guarantees a nonzero refcount.
        let ptr = unsafe { core::ptr::addr_of!((*self.0.get()).f_cred).read() };
        // SAFETY: The lifetimes of `self` and `Credential` are tied, so it is guaranteed that
        // the credential pointer remains valid (because the file is still alive, and it doesn't
        // change over the lifetime of a file).
        unsafe { Credential::from_ptr(ptr) }
    }

    /// Returns the flags associated with the file.
    pub fn flags(&self) -> u32 {
        // SAFETY: The file is valid because the shared reference guarantees a nonzero refcount.
        unsafe { core::ptr::addr_of!((*self.0.get()).f_flags).read() }
    }
}

// SAFETY: The type invariants guarantee that `File` is always ref-counted.
unsafe impl AlwaysRefCounted for File {
    fn inc_ref(&self) {
        // SAFETY: The existence of a shared reference means that the refcount is nonzero.
        unsafe { bindings::get_file(self.0.get()) };
    }

    unsafe fn dec_ref(obj: ptr::NonNull<Self>) {
        // SAFETY: The safety requirements guarantee that the refcount is nonzero.
        unsafe { bindings::fput(obj.cast().as_ptr()) }
    }
}

/// A file descriptor reservation.
///
/// This allows the creation of a file descriptor in two steps: first, we reserve a slot for it,
/// then we commit or drop the reservation. The first step may fail (e.g., the current process ran
/// out of available slots), but commit and drop never fail (and are mutually exclusive).
pub struct FileDescriptorReservation {
    fd: u32,
}

impl FileDescriptorReservation {
    /// Creates a new file descriptor reservation.
    pub fn new(flags: u32) -> Result<Self> {
        // SAFETY: FFI call, there are no safety requirements on `flags`.
        let fd = unsafe { bindings::get_unused_fd_flags(flags) };
        if fd < 0 {
            return Err(Error::from_kernel_errno(fd));
        }
        Ok(Self { fd: fd as _ })
    }

    /// Returns the file descriptor number that was reserved.
    pub fn reserved_fd(&self) -> u32 {
        self.fd
    }

    /// Commits the reservation.
    ///
    /// The previously reserved file descriptor is bound to `file`.
    pub fn commit(self, file: ARef<File>) {
        // SAFETY: `self.fd` was previously returned by `get_unused_fd_flags`, and `file.ptr` is
        // guaranteed to have an owned ref count by its type invariants.
        unsafe { bindings::fd_install(self.fd, file.0.get()) };

        // `fd_install` consumes both the file descriptor and the file reference, so we cannot run
        // the destructors.
        core::mem::forget(self);
        core::mem::forget(file);
    }
}

impl Drop for FileDescriptorReservation {
    fn drop(&mut self) {
        // SAFETY: `self.fd` was returned by a previous call to `get_unused_fd_flags`.
        unsafe { bindings::put_unused_fd(self.fd) };
    }
}

/// Wraps the kernel's `struct poll_table_struct`.
///
/// # Invariants
///
/// The pointer `PollTable::ptr` is null or valid.
pub struct PollTable {
    ptr: *mut bindings::poll_table_struct,
}

impl PollTable {
    /// Constructors a new `struct poll_table_struct` wrapper.
    ///
    /// # Safety
    ///
    /// The pointer `ptr` must be either null or a valid pointer for the lifetime of the object.
    unsafe fn from_ptr(ptr: *mut bindings::poll_table_struct) -> Self {
        Self { ptr }
    }

    /// Associates the given file and condition variable to this poll table. It means notifying the
    /// condition variable will notify the poll table as well; additionally, the association
    /// between the condition variable and the file will automatically be undone by the kernel when
    /// the file is destructed. To unilaterally remove the association before then, one can call
    /// [`CondVar::free_waiters`].
    ///
    /// # Safety
    ///
    /// If the condition variable is destroyed before the file, then [`CondVar::free_waiters`] must
    /// be called to ensure that all waiters are flushed out.
    pub unsafe fn register_wait<'a>(&self, file: &'a File, cv: &'a CondVar) {
        if self.ptr.is_null() {
            return;
        }

        // SAFETY: `PollTable::ptr` is guaranteed to be valid by the type invariants and the null
        // check above.
        let table = unsafe { &*self.ptr };
        if let Some(proc) = table._qproc {
            // SAFETY: All pointers are known to be valid.
            unsafe { proc(file.0.get() as _, cv.wait_list.get(), self.ptr) }
        }
    }
}

/// Equivalent to [`std::io::SeekFrom`].
///
/// [`std::io::SeekFrom`]: https://doc.rust-lang.org/std/io/enum.SeekFrom.html
pub enum SeekFrom {
    /// Equivalent to C's `SEEK_SET`.
    Start(u64),

    /// Equivalent to C's `SEEK_END`.
    End(i64),

    /// Equivalent to C's `SEEK_CUR`.
    Current(i64),
}

impl SeekFrom {
    pub fn into_pos_and_whence(self) -> (i64, u32) {
        match self {
            SeekFrom::Start(off) => (off as _, bindings::SEEK_SET),
            SeekFrom::End(off) => (off, bindings::SEEK_END),
            SeekFrom::Current(off) => (off, bindings::SEEK_CUR),
        }
    }
}

pub(crate) struct OperationsVtable<A, T>(marker::PhantomData<A>, marker::PhantomData<T>);

impl<A: OpenAdapter<T::OpenData>, T: Operations> OperationsVtable<A, T> {
    /// Called by the VFS when an inode should be opened.
    ///
    /// Calls `T::open` on the returned value of `A::convert`.
    ///
    /// # Safety
    ///
    /// The returned value of `A::convert` must be a valid non-null pointer and
    /// `T:open` must return a valid non-null pointer on an `Ok` result.
    unsafe extern "C" fn open_callback(
        inode: *mut bindings::inode,
        file: *mut bindings::file,
    ) -> c_types::c_int {
        from_kernel_result! {
            // SAFETY: `A::convert` must return a valid non-null pointer that
            // should point to data in the inode or file that lives longer
            // than the following use of `T::open`.
            let arg = unsafe { A::convert(inode, file) };
            // SAFETY: The C contract guarantees that `file` is valid. Additionally,
            // `fileref` never outlives this function, so it is guaranteed to be
            // valid.
            let fileref = unsafe { File::from_ptr(file) };
            // SAFETY: `arg` was previously returned by `A::convert` and must
            // be a valid non-null pointer.
            let ptr = T::open(unsafe { &*arg }, fileref)?.into_pointer();
            // SAFETY: The C contract guarantees that `private_data` is available
            // for implementers of the file operations (no other C code accesses
            // it), so we know that there are no concurrent threads/CPUs accessing
            // it (it's not visible to any other Rust code).
            unsafe { (*file).private_data = ptr as *mut c_types::c_void };
            Ok(0)
        }
    }

    unsafe extern "C" fn read_callback(
        file: *mut bindings::file,
        buf: *mut c_types::c_char,
        len: c_types::c_size_t,
        offset: *mut bindings::loff_t,
    ) -> c_types::c_ssize_t {
        from_kernel_result! {
            let mut data = unsafe { UserSlicePtr::new(buf as *mut c_types::c_void, len).writer() };
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            // No `FMODE_UNSIGNED_OFFSET` support, so `offset` must be in [0, 2^63).
            // See discussion in https://github.com/fishinabarrel/linux-kernel-module-rust/pull/113
            let read = T::read(
                f,
                unsafe { File::from_ptr(file) },
                &mut data,
                unsafe { *offset }.try_into()?,
            )?;
            unsafe { (*offset) += bindings::loff_t::try_from(read).unwrap() };
            Ok(read as _)
        }
    }

    unsafe extern "C" fn read_iter_callback(
        iocb: *mut bindings::kiocb,
        raw_iter: *mut bindings::iov_iter,
    ) -> isize {
        from_kernel_result! {
            let mut iter = unsafe { IovIter::from_ptr(raw_iter) };
            let file = unsafe { (*iocb).ki_filp };
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let read =
                T::read_iter(f, unsafe { iocb.as_mut() }.unwrap().as_mut(), &mut iter)?;
            Ok(read as _)
        }
    }

    unsafe extern "C" fn write_callback(
        file: *mut bindings::file,
        buf: *const c_types::c_char,
        len: c_types::c_size_t,
        offset: *mut bindings::loff_t,
    ) -> c_types::c_ssize_t {
        from_kernel_result! {
            let mut data = unsafe { UserSlicePtr::new(buf as *mut c_types::c_void, len).reader() };
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            // No `FMODE_UNSIGNED_OFFSET` support, so `offset` must be in [0, 2^63).
            // See discussion in https://github.com/fishinabarrel/linux-kernel-module-rust/pull/113
            let written = T::write(
                f,
                unsafe { File::from_ptr(file) },
                &mut data,
                unsafe { *offset }.try_into()?
            )?;
            unsafe { (*offset) += bindings::loff_t::try_from(written).unwrap() };
            Ok(written as _)
        }
    }

    unsafe extern "C" fn write_iter_callback(
        iocb: *mut bindings::kiocb,
        raw_iter: *mut bindings::iov_iter,
    ) -> isize {
        from_kernel_result! {
            let mut iter = unsafe { IovIter::from_ptr(raw_iter) };
            let file = unsafe { (*iocb).ki_filp };
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let written =
                T::write_iter(f, unsafe { iocb.as_mut() }.unwrap().as_mut(), &mut iter)?;
            Ok(written as _)
        }
    }

    unsafe extern "C" fn release_callback(
        _inode: *mut bindings::inode,
        file: *mut bindings::file,
    ) -> c_types::c_int {
        let ptr = mem::replace(unsafe { &mut (*file).private_data }, ptr::null_mut());
        T::release(unsafe { T::Data::from_pointer(ptr as _) }, unsafe {
            File::from_ptr(file)
        });
        0
    }

    unsafe extern "C" fn llseek_callback(
        file: *mut bindings::file,
        offset: bindings::loff_t,
        whence: c_types::c_int,
    ) -> bindings::loff_t {
        from_kernel_result! {
            let off = match whence as u32 {
                bindings::SEEK_SET => SeekFrom::Start(offset.try_into()?),
                bindings::SEEK_CUR => SeekFrom::Current(offset),
                bindings::SEEK_END => SeekFrom::End(offset),
                _ => return Err(EINVAL),
            };
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let off = T::seek(f, unsafe { File::from_ptr(file) }, off)?;
            Ok(off as bindings::loff_t)
        }
    }

    unsafe extern "C" fn unlocked_ioctl_callback(
        file: *mut bindings::file,
        cmd: c_types::c_uint,
        arg: c_types::c_ulong,
    ) -> c_types::c_long {
        from_kernel_result! {
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let mut cmd = IoctlCommand::new(cmd as _, arg as _);
            let ret = T::ioctl(f, unsafe { File::from_ptr(file) }, &mut cmd)?;
            Ok(ret as _)
        }
    }

    unsafe extern "C" fn compat_ioctl_callback(
        file: *mut bindings::file,
        cmd: c_types::c_uint,
        arg: c_types::c_ulong,
    ) -> c_types::c_long {
        from_kernel_result! {
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let mut cmd = IoctlCommand::new(cmd as _, arg as _);
            let ret = T::compat_ioctl(f, unsafe { File::from_ptr(file) }, &mut cmd)?;
            Ok(ret as _)
        }
    }

    unsafe extern "C" fn mmap_callback(
        file: *mut bindings::file,
        vma: *mut bindings::vm_area_struct,
    ) -> c_types::c_int {
        from_kernel_result! {
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };

            // SAFETY: The C API guarantees that `vma` is valid for the duration of this call.
            // `area` only lives within this call, so it is guaranteed to be valid.
            let mut area = unsafe { mm::virt::Area::from_ptr(vma) };

            // SAFETY: The C API guarantees that `file` is valid for the duration of this call,
            // which is longer than the lifetime of the file reference.
            T::mmap(f, unsafe { File::from_ptr(file) }, &mut area)?;
            Ok(0)
        }
    }

    unsafe extern "C" fn fsync_callback(
        file: *mut bindings::file,
        start: bindings::loff_t,
        end: bindings::loff_t,
        datasync: c_types::c_int,
    ) -> c_types::c_int {
        from_kernel_result! {
            let start = start.try_into()?;
            let end = end.try_into()?;
            let datasync = datasync != 0;
            // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
            // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the
            // `release` callback, which the C API guarantees that will be called only when all
            // references to `file` have been released, so we know it can't be called while this
            // function is running.
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let res = T::fsync(f, unsafe { File::from_ptr(file) }, start, end, datasync)?;
            Ok(res.try_into().unwrap())
        }
    }

    unsafe extern "C" fn get_unmapped_area_callback(
        file: *mut bindings::file,
        addr: c_types::c_ulong,
        len: c_types::c_ulong,
        pgoff: c_types::c_ulong,
        flags: c_types::c_ulong,
    ) -> c_types::c_ulong {
        let ret: i64 = from_kernel_result! {
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let res = T::get_unmapped_area(f, unsafe { File::from_ptr(file) }, addr, len, pgoff, flags)?;
            Ok(res as _)
        };
        ret as _
    }

    unsafe extern "C" fn poll_callback(
        file: *mut bindings::file,
        wait: *mut bindings::poll_table_struct,
    ) -> bindings::__poll_t {
        // SAFETY: `private_data` was initialised by `open_callback` with a value returned by
        // `T::Data::into_pointer`. `T::Data::from_pointer` is only called by the `release`
        // callback, which the C API guarantees that will be called only when all references to
        // `file` have been released, so we know it can't be called while this function is running.
        let f = unsafe { T::Data::borrow((*file).private_data) };
        match T::poll(f, unsafe { File::from_ptr(file) }, unsafe {
            &PollTable::from_ptr(wait)
        }) {
            Ok(v) => v,
            Err(_) => bindings::POLLERR,
        }
    }

    unsafe extern "C" fn splice_read_callback(
        file: *mut bindings::file,
        ppos: *mut bindings::loff_t,
        pipe: *mut bindings::pipe_inode_info,
        len: c_types::c_size_t,
        flags: c_types::c_uint,
    ) -> c_types::c_ssize_t {
        from_kernel_result! {
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let ret = T::splice_read(f, unsafe { File::from_ptr(file) }, ppos, &mut unsafe {*pipe }, len, flags)?;
            Ok(ret as _)
        }
    }

    unsafe extern "C" fn splice_write_callback(
        pipe: *mut bindings::pipe_inode_info,
        file: *mut bindings::file,
        ppos: *mut bindings::loff_t,
        len: c_types::c_size_t,
        flags: c_types::c_uint,
    ) -> c_types::c_ssize_t {
        from_kernel_result! {
            let f = unsafe { T::Data::borrow((*file).private_data) };
            let ret = T::splice_write(f, &mut unsafe { *pipe }, unsafe { File::from_ptr(file) }, ppos, len, flags)?;
            Ok(ret as _)
        }
    }

    unsafe extern "C" fn fallocate_callback(
        file: *mut bindings::file,
        mode: c_types::c_int,
        offset: bindings::loff_t,
        length: bindings::loff_t,
    ) -> c_types::c_long {
        from_kernel_result! {
            let f = unsafe { T::Data::borrow((*file).private_data) };
            T::allocate_file(f, unsafe { File::from_ptr(file) }, FileAllocMode::from_int(mode as _), offset, length).map(|()| 0)
        }
    }

    const VTABLE: bindings::file_operations = bindings::file_operations {
        open: Some(Self::open_callback),
        release: Some(Self::release_callback),
        read: if T::TO_USE.read {
            Some(Self::read_callback)
        } else {
            None
        },
        write: if T::TO_USE.write {
            Some(Self::write_callback)
        } else {
            None
        },
        llseek: if T::TO_USE.seek {
            Some(Self::llseek_callback)
        } else {
            None
        },

        check_flags: None,
        compat_ioctl: if T::TO_USE.compat_ioctl {
            Some(Self::compat_ioctl_callback)
        } else {
            None
        },
        copy_file_range: None,
        fallocate: if T::TO_USE.allocate_file {
            Some(fallocate_callback)
        } else {
            None
        },
        fadvise: None,
        fasync: None,
        flock: None,
        flush: None,
        fsync: if T::TO_USE.fsync {
            Some(Self::fsync_callback)
        } else {
            None
        },
        get_unmapped_area: if T::TO_USE.get_unmapped_area {
            Some(get_unmapped_area_callback)
        } else {
            None
        },
        iterate: None,
        iterate_shared: None,
        iopoll: None,
        lock: None,
        mmap: if T::TO_USE.mmap {
            Some(Self::mmap_callback)
        } else {
            None
        },
        mmap_supported_flags: 0,
        owner: ptr::null_mut(),
        poll: if T::TO_USE.poll {
            Some(Self::poll_callback)
        } else {
            None
        },
        read_iter: if T::TO_USE.read_iter {
            Some(Self::read_iter_callback)
        } else {
            None
        },
        remap_file_range: None,
        sendpage: None,
        setlease: None,
        show_fdinfo: None,
        splice_read: if T::TO_USE.splice_read {
            Some(splice_read_callback)
        } else {
            None
        },
        splice_write: if T::TO_USE.splice_write {
            Some(splice_write_callback)
        } else {
            None
        },
        unlocked_ioctl: if T::TO_USE.ioctl {
            Some(Self::unlocked_ioctl_callback)
        } else {
            None
        },
        write_iter: if T::TO_USE.write_iter {
            Some(Self::write_iter_callback)
        } else {
            None
        },
    };

    /// Builds an instance of [`struct file_operations`].
    ///
    /// # Safety
    ///
    /// The caller must ensure that the adapter is compatible with the way the device is registered.
    pub(crate) const unsafe fn build() -> &'static bindings::file_operations {
        &Self::VTABLE
    }
}

/// Represents which fields of [`struct file_operations`] should be populated with pointers.
pub struct ToUse {
    /// The `read` field of [`struct file_operations`].
    pub read: bool,

    /// The `read_iter` field of [`struct file_operations`].
    pub read_iter: bool,

    /// The `write` field of [`struct file_operations`].
    pub write: bool,

    /// The `write_iter` field of [`struct file_operations`].
    pub write_iter: bool,

    /// The `llseek` field of [`struct file_operations`].
    pub seek: bool,

    /// The `unlocked_ioctl` field of [`struct file_operations`].
    pub ioctl: bool,

    /// The `compat_ioctl` field of [`struct file_operations`].
    pub compat_ioctl: bool,

    /// The `fsync` field of [`struct file_operations`].
    pub fsync: bool,

    /// The `get_unmapped_area` field of [`struct file_operations`].
    pub get_unmapped_area: bool,

    /// The `mmap` field of [`struct file_operations`].
    pub mmap: bool,

    /// The `poll` field of [`struct file_operations`].
    pub poll: bool,

    /// The `splice_read` field of [`struct file_operations`].
    pub splice_read: bool,

    /// The `splice_write` field of [`struct file_operations`].
    pub splice_write: bool,

    /// The `fallocate` field of [`struct file_operations`].
    pub allocate_file: bool,
}

/// A constant version where all values are to set to `false`, that is, all supported fields will
/// be set to null pointers.
pub const USE_NONE: ToUse = ToUse {
    read: false,
    read_iter: false,
    write: false,
    write_iter: false,
    seek: false,
    ioctl: false,
    compat_ioctl: false,
    fsync: false,
    get_unmapped_area: false,
    mmap: false,
    poll: false,
    splice_read: false,
    splice_write: false,
    allocate_file: false,
};

/// Defines the [`Operations::TO_USE`] field based on a list of fields to be populated.
#[macro_export]
macro_rules! declare_file_operations {
    () => {
        const TO_USE: $crate::file::ToUse = $crate::file::USE_NONE;
    };
    ($($i:ident),+) => {
        const TO_USE: kernel::file::ToUse =
            $crate::file::ToUse {
                $($i: true),+ ,
                ..$crate::file::USE_NONE
            };
    };
}

/// Allows the handling of ioctls defined with the `_IO`, `_IOR`, `_IOW`, and `_IOWR` macros.
///
/// For each macro, there is a handler function that takes the appropriate types as arguments.
pub trait IoctlHandler: Sync {
    /// The type of the first argument to each associated function.
    type Target<'a>;

    /// Handles ioctls defined with the `_IO` macro, that is, with no buffer as argument.
    fn pure(_this: Self::Target<'_>, _file: &File, _cmd: u32, _arg: usize) -> Result<i32> {
        Err(EINVAL)
    }

    /// Handles ioctls defined with the `_IOR` macro, that is, with an output buffer provided as
    /// argument.
    fn read(
        _this: Self::Target<'_>,
        _file: &File,
        _cmd: u32,
        _writer: &mut UserSlicePtrWriter,
    ) -> Result<i32> {
        Err(EINVAL)
    }

    /// Handles ioctls defined with the `_IOW` macro, that is, with an input buffer provided as
    /// argument.
    fn write(
        _this: Self::Target<'_>,
        _file: &File,
        _cmd: u32,
        _reader: &mut UserSlicePtrReader,
    ) -> Result<i32> {
        Err(EINVAL)
    }

    /// Handles ioctls defined with the `_IOWR` macro, that is, with a buffer for both input and
    /// output provided as argument.
    fn read_write(
        _this: Self::Target<'_>,
        _file: &File,
        _cmd: u32,
        _data: UserSlicePtr,
    ) -> Result<i32> {
        Err(EINVAL)
    }
}

/// Represents an ioctl command.
///
/// It can use the components of an ioctl command to dispatch ioctls using
/// [`IoctlCommand::dispatch`].
pub struct IoctlCommand {
    cmd: u32,
    arg: usize,
    user_slice: Option<UserSlicePtr>,
}

impl IoctlCommand {
    /// Constructs a new [`IoctlCommand`].
    fn new(cmd: u32, arg: usize) -> Self {
        let size = (cmd >> bindings::_IOC_SIZESHIFT) & bindings::_IOC_SIZEMASK;

        // SAFETY: We only create one instance of the user slice per ioctl call, so TOCTOU issues
        // are not possible.
        let user_slice = Some(unsafe { UserSlicePtr::new(arg as _, size as _) });
        Self {
            cmd,
            arg,
            user_slice,
        }
    }

    /// Dispatches the given ioctl to the appropriate handler based on the value of the command. It
    /// also creates a [`UserSlicePtr`], [`UserSlicePtrReader`], or [`UserSlicePtrWriter`]
    /// depending on the direction of the buffer of the command.
    ///
    /// It is meant to be used in implementations of [`Operations::ioctl`] and
    /// [`Operations::compat_ioctl`].
    pub fn dispatch<T: IoctlHandler>(
        &mut self,
        handler: T::Target<'_>,
        file: &File,
    ) -> Result<i32> {
        let dir = (self.cmd >> bindings::_IOC_DIRSHIFT) & bindings::_IOC_DIRMASK;
        if dir == bindings::_IOC_NONE {
            return T::pure(handler, file, self.cmd, self.arg);
        }

        let data = self.user_slice.take().ok_or(EINVAL)?;
        const READ_WRITE: u32 = bindings::_IOC_READ | bindings::_IOC_WRITE;
        match dir {
            bindings::_IOC_WRITE => T::write(handler, file, self.cmd, &mut data.reader()),
            bindings::_IOC_READ => T::read(handler, file, self.cmd, &mut data.writer()),
            READ_WRITE => T::read_write(handler, file, self.cmd, data),
            _ => Err(EINVAL),
        }
    }

    /// Returns the raw 32-bit value of the command and the ptr-sized argument.
    pub fn raw(&self) -> (u32, usize) {
        (self.cmd, self.arg)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileAllocMode(u8);

#[rustfmt::skip]
impl FileAllocMode {
    pub const KEEP_SIZE: Self      = Self::from_int(0x01);
    pub const PUNCH_HOLE: Self     = Self::from_int(0x02);
    pub const NO_HIDE_STALE: Self  = Self::from_int(0x04);
    pub const COLLAPSE_RANGE: Self = Self::from_int(0x08);
    pub const ZERO_RANGE: Self     = Self::from_int(0x10);
    pub const INSERT_RANGE: Self   = Self::from_int(0x20);
    pub const UNSHARE_RANGE: Self  = Self::from_int(0x40);
}

#[derive(Debug, Clone, Copy)]
pub struct FileTimeFlags(u8);

#[rustfmt::skip]
impl FileTimeFlags {
    pub const A: Self       = Self::from_int(1);
    pub const M: Self       = Self::from_int(2);
    pub const C: Self       = Self::from_int(4);
    pub const VERSION: Self = Self::from_int(8);
}

#[derive(Debug, Clone, Copy)]
pub struct FMode(u32);

impl FMode {
    // copied from include/linux/fs.h
    /* file is open for reading */
    pub const FMODE_READ: Self = Self::from_int(0x1);
    /* file is open for writing */
    pub const FMODE_WRITE: Self = Self::from_int(0x2);
    /* file is seekable */
    pub const FMODE_LSEEK: Self = Self::from_int(0x4);
    /* file can be accessed using pread */
    pub const FMODE_PREAD: Self = Self::from_int(0x8);
    /* file can be accessed using pwrite */
    pub const FMODE_PWRITE: Self = Self::from_int(0x10);
    /* File is opened for execution with sys_execve / sys_uselib */
    pub const FMODE_EXEC: Self = Self::from_int(0x20);
    /* File is opened with O_NDELAY (only set for block devices) */
    pub const FMODE_NDELAY: Self = Self::from_int(0x40);
    /* File is opened with O_EXCL (only set for block devices) */
    pub const FMODE_EXCL: Self = Self::from_int(0x80);
    /* File is opened using open(.., 3, ..) and is writeable only for ioctls
    (specialy hack for floppy.c) */
    pub const FMODE_WRITE_IOCTL: Self = Self::from_int(0x100);
    /* 32bit hashes as llseek() offset (for directories) */
    pub const FMODE_32BITHASH: Self = Self::from_int(0x200);
    /* 64bit hashes as llseek() offset (for directories) */
    pub const FMODE_64BITHASH: Self = Self::from_int(0x400);
    /*
     * Don't update ctime and mtime.
     *
     * Currently a special hack for the XFS open_by_handle ioctl, but we'll
     * hopefully graduate it to a proper O_CMTIME flag supported by open(2) soon.
     */
    pub const FMODE_NOCMTIME: Self = Self::from_int(0x800);
    /* Expect random access pattern */
    pub const FMODE_RANDOM: Self = Self::from_int(0x1000);
    /* File is huge (eg. /dev/kmem): treat loff_t as unsigned */
    pub const FMODE_UNSIGNED_OFFSET: Self = Self::from_int(0x2000);
    /* File is opened with O_PATH; almost nothing can be done with it */
    pub const FMODE_PATH: Self = Self::from_int(0x4000);
    /* File needs atomic accesses to f_pos */
    pub const FMODE_ATOMIC_POS: Self = Self::from_int(0x8000);
    /* Write access to underlying fs */
    pub const FMODE_WRITER: Self = Self::from_int(0x10000);
    /* Has read method(s) */
    pub const FMODE_CAN_READ: Self = Self::from_int(0x20000);
    /* Has write method(s) */
    pub const FMODE_CAN_WRITE: Self = Self::from_int(0x40000);
    pub const FMODE_OPENED: Self = Self::from_int(0x80000);
    pub const FMODE_CREATED: Self = Self::from_int(0x100000);
    /* File is stream-like */
    pub const FMODE_STREAM: Self = Self::from_int(0x200000);
    /* File was opened by fanotify and shouldn't generate fanotify events */
    pub const FMODE_NONOTIFY: Self = Self::from_int(0x4000000);
    /* File is capable of returning -EAGAIN if I/O will block */
    pub const FMODE_NOWAIT: Self = Self::from_int(0x8000000);
    /* File represents mount that needs unmounting */
    pub const FMODE_NEED_UNMOUNT: Self = Self::from_int(0x10000000);
    /* File does not contribute to nr_files count */
    pub const FMODE_NOACCOUNT: Self = Self::from_int(0x20000000);
    /* File supports async buffered reads */
    pub const FMODE_BUF_RASYNC: Self = Self::from_int(0x40000000);
}

macro_rules! impl_flag_methods {
    ($T:ty, $V:ty) => {
        impl $T {
            pub const fn empty() -> Self {
                Self(0)
            }
            pub const fn from_int(val: $V) -> Self {
                Self(val)
            }
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }
            pub const fn has(self, other: Self) -> bool {
                self.0 & other.0 != 0
            }
            pub const fn with(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
            pub const fn without(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }
        }
    };
}

impl_flag_methods!(FileAllocMode, u8);
impl_flag_methods!(FileTimeFlags, u8);
impl_flag_methods!(FMode, u32);

/// Trait for extracting file open arguments from kernel data structures.
///
/// This is meant to be implemented by registration managers.
pub trait OpenAdapter<T: Sync> {
    /// Converts untyped data stored in [`struct inode`] and [`struct file`] (when [`struct
    /// file_operations::open`] is called) into the given type. For example, for `miscdev`
    /// devices, a pointer to the registered [`struct miscdev`] is stored in [`struct
    /// file::private_data`].
    ///
    /// # Safety
    ///
    /// This function must be called only when [`struct file_operations::open`] is being called for
    /// a file that was registered by the implementer. The returned pointer must be valid and
    /// not-null.
    unsafe fn convert(_inode: *mut bindings::inode, _file: *mut bindings::file) -> *const T;
}

pub struct NopOpenAdapter;
impl OpenAdapter<()> for NopOpenAdapter {
    unsafe fn convert(_inode: *mut bindings::inode, _file: *mut bindings::file) -> *const () {
        &()
    }
}

impl<T: Operations<OpenData = ()>> BuildVtable<bindings::file_operations> for T {
    fn build_vtable() -> &'static bindings::file_operations {
        unsafe { OperationsVtable::<NopOpenAdapter, T>::build() }
    }
}

/// Corresponds to the kernel's `struct file_operations`.
///
/// You implement this trait whenever you would create a `struct file_operations`.
///
/// File descriptors may be used from multiple threads/processes concurrently, so your type must be
/// [`Sync`]. It must also be [`Send`] because [`Operations::release`] will be called from the
/// thread that decrements that associated file's refcount to zero.
pub trait Operations {
    /// The methods to use to populate [`struct file_operations`].
    const TO_USE: ToUse;

    /// The type of the context data returned by [`Operations::open`] and made available to
    /// other methods.
    type Data: PointerWrapper + Send + Sync = ();

    /// The type of the context data passed to [`Operations::open`].
    type OpenData: Sync = ();

    /// Creates a new instance of this file.
    ///
    /// Corresponds to the `open` function pointer in `struct file_operations`.
    fn open(context: &Self::OpenData, file: &File) -> Result<Self::Data>;

    /// Cleans up after the last reference to the file goes away.
    ///
    /// Note that context data is moved, so it will be freed automatically unless the
    /// implementation moves it elsewhere.
    ///
    /// Corresponds to the `release` function pointer in `struct file_operations`.
    fn release(_data: Self::Data, _file: &File) {}

    /// Reads data from this file to the caller's buffer.
    ///
    /// Corresponds to the `read` and `read_iter` function pointers in `struct file_operations`.
    fn read(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _writer: &mut impl IoBufferWriter,
        _offset: u64,
    ) -> Result<usize> {
        Err(EINVAL)
    }

    /// Reads data from this file to the caller's buffer.
    ///
    /// Corresponds to the `read_iter` function pointer in `struct file_operations`.
    fn read_iter(
        data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        iocb: &mut Kiocb,
        iter: &mut IovIter,
    ) -> Result<usize> {
        let file = iocb.get_file();
        let offset = iocb.get_offset();
        let read = Self::read(data, &file, iter, offset)?;
        let offset = iocb.get_offset();
        iocb.set_offset(offset + read as u64);
        Ok(read)
    }

    /// Writes data from the caller's buffer to this file.
    ///
    /// Corresponds to the `write` and `write_iter` function pointers in `struct file_operations`.
    fn write(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _reader: &mut impl IoBufferReader,
        _offset: u64,
    ) -> Result<usize> {
        Err(EINVAL)
    }

    /// Writes data from the caller's buffer to this file.
    ///
    /// Corresponds to the `write_iter` function pointer in `struct file_operations`.
    fn write_iter(
        data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        iocb: &mut Kiocb,
        iter: &mut IovIter,
    ) -> Result<usize> {
        let file = iocb.get_file();
        let offset = iocb.get_offset();
        let written = Self::write(data, &file, iter, offset)?;
        let offset = iocb.get_offset();
        iocb.set_offset(offset + written as u64);
        Ok(written)
    }

    /// Changes the position of the file.
    ///
    /// Corresponds to the `llseek` function pointer in `struct file_operations`.
    fn seek(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _offset: SeekFrom,
    ) -> Result<u64> {
        Err(EINVAL)
    }

    /// Performs IO control operations that are specific to the file.
    ///
    /// Corresponds to the `unlocked_ioctl` function pointer in `struct file_operations`.
    fn ioctl(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _cmd: &mut IoctlCommand,
    ) -> Result<i32> {
        Err(ENOTTY)
    }

    /// Performs 32-bit IO control operations on that are specific to the file on 64-bit kernels.
    ///
    /// Corresponds to the `compat_ioctl` function pointer in `struct file_operations`.
    fn compat_ioctl(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _cmd: &mut IoctlCommand,
    ) -> Result<i32> {
        Err(ENOTTY)
    }

    /// Syncs pending changes to this file.
    ///
    /// Corresponds to the `fsync` function pointer in `struct file_operations`.
    fn fsync(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _start: u64,
        _end: u64,
        _datasync: bool,
    ) -> Result<u32> {
        Err(EINVAL)
    }

    /// Unmapped area.
    ///
    /// Corresponds to the `get_unmapped_area` function pointer in `struct file_operations`.
    fn get_unmapped_area(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _addr: u64,
        _len: u64,
        _pgoff: u64,
        _flags: u64,
    ) -> Result<u64> {
        Err(Error::EINVAL)
    }

    /// Maps areas of the caller's virtual memory with device/file memory.
    ///
    /// Corresponds to the `mmap` function pointer in `struct file_operations`.
    fn mmap(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _vma: &mut mm::virt::Area,
    ) -> Result {
        Err(EINVAL)
    }

    /// Checks the state of the file and optionally registers for notification when the state
    /// changes.
    ///
    /// Corresponds to the `poll` function pointer in `struct file_operations`.
    fn poll(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _table: &PollTable,
    ) -> Result<u32> {
        Ok(bindings::POLLIN | bindings::POLLOUT | bindings::POLLRDNORM | bindings::POLLWRNORM)
    }

    /// Splice data from file to a pipe
    ///
    /// Corresponds to the `splice_read` function pointer in `struct file_operations`.
    fn splice_read(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _pos: *mut i64,
        _pipe: &mut bindings::pipe_inode_info,
        _len: usize,
        _flags: u32,
    ) -> Result<usize> {
        Err(Error::EINVAL)
    }

    /// Splice data from pipe to a file
    ///
    /// Corresponds to the `splice_write` function pointer in `struct file_operations`.
    fn splice_write(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _pipe: &mut bindings::pipe_inode_info,
        _file: &File,
        _pos: *mut i64,
        _len: usize,
        _flags: u32,
    ) -> Result<usize> {
        Err(Error::EINVAL)
    }

    /// Preallocate space for a file
    ///
    /// Corresponds to the `fallocate` function pointer in `struct file_operations`.
    fn allocate_file(
        _data: <Self::Data as PointerWrapper>::Borrowed<'_>,
        _file: &File,
        _mode: FileAllocMode,
        _offset: bindings::loff_t,
        _length: bindings::loff_t,
    ) -> Result {
        Err(Error::EINVAL)
    }
}
