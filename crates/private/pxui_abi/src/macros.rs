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
