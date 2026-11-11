/// Declares the library functions one wrapper module calls. Linked statically they are the `pxui_abi` items, and
/// each signature written here is checked against the real one; linked dynamically they are one `extern` block.
/// Either way the module `$m` holds exactly the names listed. A `#[trusted]` one is, linked statically, the library's trusted
/// entry of that name (a Rust function without the checks of untrusted input: this module's arguments are valid Rust values).
macro_rules! library {
    (@check [] $name:ident ($($ty:ty),*) $(-> $ret:ty)?) => { const _: unsafe extern "C" fn($($ty),*) $(-> $ret)? = $name; };
    (@check [trusted] $name:ident ($($ty:ty),*) $(-> $ret:ty)?) => { const _: unsafe fn($($ty),*) $(-> $ret)? = $name; };
    ($m:ident { $($(#[$t:ident])? fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)* }) => {
        #[cfg(feature = "static")]
        mod $m {
            #[allow(unused_imports)] use super::*;
            #[allow(unused_imports)] use crate::sys::PxuiUi;
            $(pub use pxui_abi_link::$($t::)? $name;
            library!(@check [$($t)?] $name ($($ty),*) $(-> $ret)?);)*
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

/// The closure the static wrapper hands a trusted container entry for its children: the borrowed handle becomes a wrapper `Ui` for
/// `$f`. No trampoline, no catch: a panic of `$f` unwinds through the library to the app.
#[cfg(feature = "static")]
macro_rules! inner {
    ($f:expr) => { &mut |raw: *mut crate::sys::PxuiUi| { let mut ui = core::mem::ManuallyDrop::new(Ui(core::ptr::NonNull::new_unchecked(raw))); $f(&mut ui) } };
}
