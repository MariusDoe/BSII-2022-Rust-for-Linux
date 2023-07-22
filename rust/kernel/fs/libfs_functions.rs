//! TODO: docs
use crate::{
    bindings,
    error::{Result, to_result},
    str::CStr,
    fs::{INode,Type}
    
};

/// TODO: docs
pub fn page_symlink<T: Type + ?Sized>(inode: &mut INode<T>, symname: &'static CStr) -> Result {
    to_result(unsafe {
        bindings::page_symlink(
            inode.0.get(),
            symname.as_ptr() as _,
            symname.len_with_nul() as _,
        )
    })
    .map(|_| ())
}
