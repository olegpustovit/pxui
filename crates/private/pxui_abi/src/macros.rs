/// Every `pxui_*` export is declared through this macro: `export!(pub unsafe fn pxui_x(ui: *mut PxuiUi, ..) -> i32 { body });`.
/// The body runs under `catch_unwind`, so no unwinding crosses the C boundary. A caught panic
/// poisons the `Ui` reached through the first argument and returns `PXUI_ERR_PANIC` (status calls)
/// or the zero value; while poisoned every later call on that `Ui` returns the same at once.
/// Forms: `bare` has no handle (create, destroy, version, free functions); `frame` is the two
/// frame entry points, which clear the poison flag themselves; the default form takes the handle
/// as its first argument.
macro_rules! export {
    (bare $vis:vis unsafe fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> i32 $body:block) => {
        export!(@bare $vis $name ($($arg: $ty),*) (-> i32) ($crate::PXUI_ERR_PANIC) $body);
    };
    (bare $vis:vis unsafe fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
        export!(@bare $vis $name ($($arg: $ty),*) ($(-> $ret)?) (unsafe { core::mem::zeroed() }) $body);
    };
    (frame $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> i32 $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($h: $hty $(, $arg: $ty)*) -> i32 {
            let shared = <$hty as $crate::state::Handle>::shared($h);
            export!(@run payload => { unsafe { $crate::state::panicked(shared, payload, true); } $crate::PXUI_ERR_PANIC }, $body)
        }
    };
    ($vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> i32 $body:block) => {
        export!(@handle $vis $name ($h: $hty $(, $arg: $ty)*) (-> i32) ($crate::PXUI_ERR_PANIC) $body);
    };
    ($vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)? $body:block) => {
        export!(@handle $vis $name ($h: $hty $(, $arg: $ty)*) ($(-> $ret)?) (unsafe { core::mem::zeroed() }) $body);
    };
    (@bare $vis:vis $name:ident ($($arg:ident: $ty:ty),*) ($($ret:tt)*) ($failed:expr) $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($($arg: $ty),*) $($ret)* {
            export!(@run _payload => { $failed }, $body)
        }
    };
    (@handle $vis:vis $name:ident ($h:ident: $hty:ty $(, $arg:ident: $ty:ty)*) ($($ret:tt)*) ($failed:expr) $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($h: $hty $(, $arg: $ty)*) $($ret)* {
            let shared = <$hty as $crate::state::Handle>::shared($h);
            if unsafe { $crate::state::poisoned(shared) } { return $failed; }
            export!(@run payload => { unsafe { $crate::state::panicked(shared, payload, false); } $failed }, $body)
        }
    };
    // the one place a panic is caught
    (@run $payload:ident => $on_panic:block, $body:block) => {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)) {
            Ok(value) => value,
            Err($payload) => $on_panic,
        }
    };
}

/// A copy of the descriptor behind `$ptr` (of type `$ty`, whose first field is `size: u32`): the size is read before
/// anything else, a descriptor shorter than `$ty` is refused (`None`) and at most `size_of::<$ty>()` bytes are copied,
/// so a caller that grew its struct later is fine and a shorter one is never over-read.
macro_rules! read_desc {
    ($ptr:expr, $ty:ty) => {{
        let p: *const $ty = $ptr;
        let size = unsafe { p.cast::<u32>().read_unaligned() } as usize;
        if size < core::mem::size_of::<$ty>() { None } else {
            let mut value: $ty = unsafe { core::mem::zeroed() };
            unsafe { core::ptr::copy_nonoverlapping(p.cast::<u8>(), (&mut value as *mut $ty).cast::<u8>(), core::mem::size_of::<$ty>()); }
            Some(value)
        }
    }};
}

/// `Some(&*p)` when `p` (a pointer to a descriptor whose first field is `size: u32`) is not null and its `size` covers the
/// descriptor; a shorter record is refused before anything else is read. The pointee's size is the distance to the next element.
macro_rules! sized {
    ($ptr:expr) => {{
        let p = $ptr;
        if p.is_null() || unsafe { p.cast::<u32>().read_unaligned() } < (p.wrapping_add(1) as usize).wrapping_sub(p as usize) as u32 { None } else { Some(unsafe { &*p }) }
    }};
}

/// `sized!` for a descriptor the call writes.
macro_rules! sized_mut {
    ($ptr:expr) => {{
        let p = $ptr;
        if p.is_null() || unsafe { p.cast::<u32>().read_unaligned() } < (p.wrapping_add(1) as usize).wrapping_sub(p as usize) as u32 { None } else { Some(unsafe { &mut *p }) }
    }};
}
