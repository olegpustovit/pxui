/// Declares the library functions one wrapper module calls. Linked statically they are the `pxui_abi` items, and
/// each signature written here is checked against the real one; linked dynamically they are one `extern` block.
/// Either way the module `$m` holds exactly the names listed.
macro_rules! library {
    ($m:ident { $(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)* }) => {
        #[cfg(feature = "static")]
        mod $m {
            #[allow(unused_imports)] use super::*;
            #[allow(unused_imports)] use crate::sys::PxuiUi;
            $(pub use pxui_abi_link::$name;
            const _: unsafe extern "C" fn($($ty),*) $(-> $ret)? = $name;)*
        }
        #[cfg(not(feature = "static"))]
        mod $m {
            #[allow(unused_imports)] use super::*;
            #[allow(unused_imports)] use crate::sys::PxuiUi;
            #[link(name = "pxui")]
            unsafe extern "C" { $(pub fn $name($($arg: $ty),*) $(-> $ret)?;)* }
        }
    };
}
