/// Every `pxui_*` export is declared through this macro: `export!(pub unsafe fn pxui_x(ui: *mut PxuiUi, ..) -> i32 { body });`.
/// The body runs under `catch_unwind`, so no unwinding crosses the C boundary. A caught panic
/// poisons the `Ui` reached through the first argument and returns `PXUI_ERR_PANIC` (status calls)
/// or the zero value; while poisoned every later call on that `Ui` returns the same at once.
/// Forms: `bare` has no handle (create, destroy, version, free functions); `object` takes an object without a `Ui` (a shaper,
/// a theme, an accessibility bridge) as its first argument and poisons that object as the default form poisons a `Ui`, so a
/// half-mutated object is never used again (its destroy, a `bare` export, still frees it); `frame` is the two
/// frame entry points, which clear the poison flag themselves; the default form takes the handle
/// as its first argument.
macro_rules! export {
    (object $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> i32 $body:block) => {
        export!(@object $vis $name ($h: $hty $(, $arg: $ty)*) (-> i32) ($crate::PXUI_ERR_PANIC) $body);
    };
    (object failing($failed:expr) $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> $ret:ty $body:block) => {
        export!(@object $vis $name ($h: $hty $(, $arg: $ty)*) (-> $ret) ($failed) $body);
    };
    (object $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)? $body:block) => {
        export!(@object $vis $name ($h: $hty $(, $arg: $ty)*) ($(-> $ret)?) (unsafe { core::mem::zeroed() }) $body);
    };
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
            unsafe { $crate::state::note_call(shared) };
            export!(@run payload => { unsafe { $crate::state::panicked(shared, payload, true); } $crate::PXUI_ERR_PANIC }, $body)
        }
    };
    // `build`: a builder (it declares nodes of the frame being built) refused outside a frame with `PXUI_ERR_STATE`, as `enter!` refuses a
    // trusted one: the owner handle is usable between frames, the engine builds only in a frame callback
    (build $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> i32 $body:block) => {
        export!(@handle true, $vis $name ($h: $hty $(, $arg: $ty)*) (-> i32) ($crate::PXUI_ERR_PANIC) (status) {
            if !$h.is_null() && !unsafe { (*(*$h).shared).in_frame } { return unsafe { $crate::state::outside_frame((*$h).shared) }; }
            $body
        });
    };
    // `trusted`: the body ends in a call of a `trusted` entry, which counts the call itself (the static wrapper calls it directly)
    (trusted frame $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> i32 $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($h: $hty $(, $arg: $ty)*) -> i32 {
            let shared = <$hty as $crate::state::Handle>::shared($h);
            export!(@run payload => { unsafe { $crate::state::panicked(shared, payload, true); } $crate::PXUI_ERR_PANIC }, $body)
        }
    };
    (trusted $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> i32 $body:block) => {
        export!(@handle false, $vis $name ($h: $hty $(, $arg: $ty)*) (-> i32) ($crate::PXUI_ERR_PANIC) (status) $body);
    };
    (trusted $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)? $body:block) => {
        export!(@handle false, $vis $name ($h: $hty $(, $arg: $ty)*) ($(-> $ret)?) (unsafe { core::mem::zeroed() }) (value) $body);
    };
    // `failing`: a value export whose zero is a real answer (a count, an index) answers its own error value `$failed` on refusal and panic
    (failing($failed:expr) $vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> $ret:ty $body:block) => {
        export!(@handle true, $vis $name ($h: $hty $(, $arg: $ty)*) (-> $ret) ($failed) ([$failed]) $body);
    };
    ($vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> i32 $body:block) => {
        export!(@handle true, $vis $name ($h: $hty $(, $arg: $ty)*) (-> i32) ($crate::PXUI_ERR_PANIC) (status) $body);
    };
    ($vis:vis unsafe fn $name:ident($h:ident: $hty:ty $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)? $body:block) => {
        export!(@handle true, $vis $name ($h: $hty $(, $arg: $ty)*) ($(-> $ret)?) (unsafe { core::mem::zeroed() }) (value) $body);
    };
    (@bare $vis:vis $name:ident ($($arg:ident: $ty:ty),*) ($($ret:tt)*) ($failed:expr) $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($($arg: $ty),*) $($ret)* {
            export!(@run _payload => { $failed }, $body)
        }
    };
    (@object $vis:vis $name:ident ($h:ident: $hty:ty $(, $arg:ident: $ty:ty)*) ($($ret:tt)*) ($failed:expr) $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($h: $hty $(, $arg: $ty)*) $($ret)* {
            let poison = <$hty as $crate::state::Poison>::poison($h);
            // a poisoned object answers its failure value at once; null has no flag (the body refuses it)
            if !poison.is_null() && unsafe { *poison } { return $failed; }
            export!(@run _payload => { if !poison.is_null() { unsafe { *poison = true; } } $failed }, $body)
        }
    };
    (@handle $count:tt, $vis:vis $name:ident ($h:ident: $hty:ty $(, $arg:ident: $ty:ty)*) ($($ret:tt)*) ($failed:expr) ($refused:tt) $body:block) => {
        #[unsafe(no_mangle)]
        $vis unsafe extern "C" fn $name($h: $hty $(, $arg: $ty)*) $($ret)* {
            let shared = <$hty as $crate::state::Handle>::shared($h);
            // one compare: a poisoned `Ui` has no current handle, so it is refused here too (`state::refuse` tells which). A `trusted`
            // export (`$count` false) is checked by its trusted entry instead, which the static wrapper calls directly (`enter!`)
            if $count { unsafe { $crate::state::note_call(shared) }; }
            if $count && !<$hty as $crate::state::Handle>::usable($h, shared) { export!(@refuse $refused, shared); }
            export!(@run payload => { unsafe { $crate::state::panicked(shared, payload, false); } $failed }, $body)
        }
    };
    (@refuse status, $shared:expr) => { return unsafe { $crate::state::refuse($shared) } };
    (@refuse value, $shared:expr) => { { unsafe { $crate::state::refuse($shared); } return unsafe { core::mem::zeroed() }; } };
    (@refuse [$failed:expr], $shared:expr) => { { unsafe { $crate::state::refuse($shared); } return $failed; } };
    // the one place a panic is caught
    (@run $payload:ident => $on_panic:block, $body:block) => {
        {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)) {
                Ok(value) => value,
                Err($payload) => $on_panic,
            }
        }
    };
}

/// A copy of the descriptor behind `$ptr` (of type `$ty`, whose first field is `size: u32`): the size is read before
/// anything else, a descriptor shorter than `$ty` is refused (`None`) unless it is a size the record has had and at most `size_of::<$ty>()` bytes are copied,
/// so a caller that grew its struct later is fine and a shorter one is never over-read.
macro_rules! read_desc {
    // a record that grew at its end: only a `size` the record has had (`$old`, each earlier layout, listed oldest first) or the whole record
    // and more is read; a size that ends inside a field is refused. The bytes it covers are copied and every grown field is zero.
    // `Some((value, grown))`, `grown` false for an earlier layout: its grown fields (and pointers) are never read.
    ($ptr:expr, $ty:ty, [$($old:expr),+]) => {{
        let p: *const $ty = $ptr;
        let size = unsafe { p.cast::<u32>().read_unaligned() } as usize;
        let full = core::mem::size_of::<$ty>();
        if size < full && !(false $(|| size == $old as usize)+) { None } else {
            let mut value: $ty = unsafe { core::mem::zeroed() };
            unsafe { core::ptr::copy_nonoverlapping(p.cast::<u8>(), (&mut value as *mut $ty).cast::<u8>(), size.min(full)); }
            Some((value, size >= full))
        }
    }};
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

/// The start of a trusted entry: refuses a null handle (`PXUI_ERR_ARGUMENT`), counts the call and refuses a handle that is not the current one (`Shared::current`, which a poisoned
/// `Ui` has none of) before the engine is touched, with the status `state::refuse` gives, or `$refused` for an entry that returns a value.
/// A builder (the default) is refused outside a frame too, with `PXUI_ERR_STATE` (`state::outside_frame`): the engine declares nodes only
/// while it builds, and the owner handle is usable between frames. `any` is an entry that also runs between frames (a query).
macro_rules! enter {
    ($ui:expr) => {
        if $ui.is_null() { return $crate::PXUI_ERR_ARGUMENT; }
        let shared = unsafe { (*$ui).shared };
        unsafe { $crate::state::note_call(shared) };
        if !unsafe { $crate::state::usable($ui) } { return unsafe { $crate::state::refuse(shared) }; }
        if !unsafe { (*shared).in_frame } { return unsafe { $crate::state::outside_frame(shared) }; }
    };
    ($ui:expr, $refused:expr) => {
        if $ui.is_null() { return $refused; }
        let shared = unsafe { (*$ui).shared };
        unsafe { $crate::state::note_call(shared) };
        if !unsafe { $crate::state::usable($ui) } { unsafe { $crate::state::refuse(shared); } return $refused; }
        if !unsafe { (*shared).in_frame } { unsafe { $crate::state::outside_frame(shared); } return $refused; }
    };
    (any $ui:expr) => {
        if $ui.is_null() { return $crate::PXUI_ERR_ARGUMENT; }
        unsafe { $crate::state::note_call((*$ui).shared) };
        if !unsafe { $crate::state::usable($ui) } { return unsafe { $crate::state::refuse((*$ui).shared) }; }
    };
    (any $ui:expr, $refused:expr) => {
        if $ui.is_null() { return $refused; }
        unsafe { $crate::state::note_call((*$ui).shared) };
        if !unsafe { $crate::state::usable($ui) } { unsafe { $crate::state::refuse((*$ui).shared); } return $refused; }
    };
}
