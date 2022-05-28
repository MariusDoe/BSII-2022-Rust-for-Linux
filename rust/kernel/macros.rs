#![allow(dead_code)]

pub(crate) const DO_NEGATE: bool = true;
pub(crate) const DO_NOT_NEGATE: bool = false;

#[macro_export]
macro_rules! declare_constant_from_bindings {
    ($name:ident, $doc:expr) => {
        #[doc=$doc]
        #[allow(unused)]
        pub const $name: Self = Self(crate::bindings::$name as _);
    };
    ($name:ident, $doc:expr, $intermediate:ty, $negate:expr) => {
        #[doc=$doc]
        #[allow(unused)]
        pub const $name: Self =
            Self(if $negate { -1 } else { 1 } * (crate::bindings::$name as $intermediate));
    };
}
