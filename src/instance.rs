// Single-instance guard for the background watcher (`rzr --watch`), so the
// GUI can tell whether a watcher is already taking care of the headset.

#[cfg(windows)]
extern "system" {
    fn CreateMutexW(attrs: *mut u8, initial_owner: i32, name: *const u16) -> *mut u8;
    fn OpenMutexW(desired_access: u32, inherit: i32, name: *const u16) -> *mut u8;
    fn CloseHandle(handle: *mut u8) -> i32;
    fn GetLastError() -> u32;
}

#[cfg(windows)]
const ERROR_ALREADY_EXISTS: u32 = 183;

#[cfg(windows)]
fn mutex_name() -> Vec<u16> {
    "Global\\rzr_blackshark_v2_pro\0".encode_utf16().collect()
}

/// Ensure only one instance of rzr --watch is running.
/// Returns false if another instance already holds the mutex.
#[cfg(windows)]
pub fn acquire_watcher() -> bool {
    let name = mutex_name();
    unsafe {
        let handle = CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
        if handle.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
            return false;
        }
    }
    // Leak the handle — lives for process lifetime
    true
}

/// Whether a `rzr --watch` process is running.
#[cfg(windows)]
pub fn watcher_running() -> bool {
    const SYNCHRONIZE: u32 = 0x0010_0000;
    let name = mutex_name();
    unsafe {
        let handle = OpenMutexW(SYNCHRONIZE, 0, name.as_ptr());
        if handle.is_null() {
            return false;
        }
        CloseHandle(handle);
    }
    true
}

#[cfg(not(windows))]
pub fn acquire_watcher() -> bool {
    true
}

#[cfg(not(windows))]
pub fn watcher_running() -> bool {
    false
}
