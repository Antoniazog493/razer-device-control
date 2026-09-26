// Single-instance guard for the background watcher (`rzr --watch`), so the
// GUI can tell whether a watcher is already taking care of the headset, and
// a cross-process lock on the dongle's command channel.

#[cfg(windows)]
extern "system" {
    fn CreateMutexW(attrs: *mut u8, initial_owner: i32, name: *const u16) -> *mut u8;
    fn OpenMutexW(desired_access: u32, inherit: i32, name: *const u16) -> *mut u8;
    fn CloseHandle(handle: *mut u8) -> i32;
    fn GetLastError() -> u32;
    fn WaitForSingleObject(handle: *mut u8, ms: u32) -> u32;
    fn ReleaseMutex(handle: *mut u8) -> i32;
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
    mutex_exists(&mutex_name())
}

#[cfg(windows)]
fn panel_mutex_name() -> Vec<u16> {
    "Local\\rzr_panel\0".encode_utf16().collect()
}

/// Tell the watcher a panel is open, for the rest of this process: the
/// panel follows the headset's EQ button then, and the watcher leaves it be.
#[cfg(windows)]
pub fn mark_panel_open() {
    let name = panel_mutex_name();
    // Leak the handle — lives for process lifetime
    unsafe {
        CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
    }
}

/// Whether a panel (not the demo) is open.
#[cfg(windows)]
pub fn panel_open() -> bool {
    mutex_exists(&panel_mutex_name())
}

#[cfg(windows)]
fn mutex_exists(name: &[u16]) -> bool {
    const SYNCHRONIZE: u32 = 0x0010_0000;
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

#[cfg(not(windows))]
pub fn mark_panel_open() {}

#[cfg(not(windows))]
pub fn panel_open() -> bool {
    false
}

/// Lock on the dongle's command channel, shared by every rzr process.
///
/// The panel and the background watcher each keep the dongle open. Without
/// this, a query from one (which ends with remote mode off) can land in the
/// middle of the other's write sequence, and the headset then ignores the
/// rest of it. Windows mutexes are recursive per thread, so nested
/// operations just lock again.
pub struct BusLock {
    #[cfg(windows)]
    handle: *mut u8,
}

/// Held lock; released on drop.
pub struct BusGuard {
    #[cfg(windows)]
    handle: *mut u8,
}

#[cfg(windows)]
impl BusLock {
    pub fn new() -> Self {
        let name: Vec<u16> = "Local\\rzr_hid_bus\0".encode_utf16().collect();
        let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
        Self { handle }
    }

    /// Wait up to `timeout_ms` for the channel. None = another process kept
    /// it busy that long.
    pub fn lock(&self, timeout_ms: u32) -> Option<BusGuard> {
        const WAIT_OBJECT_0: u32 = 0;
        const WAIT_ABANDONED: u32 = 0x80; // previous owner died; we own it now
        if self.handle.is_null() {
            return Some(BusGuard { handle: self.handle });
        }
        match unsafe { WaitForSingleObject(self.handle, timeout_ms) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Some(BusGuard { handle: self.handle }),
            _ => None,
        }
    }
}

#[cfg(windows)]
impl Drop for BusGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ReleaseMutex(self.handle) };
        }
    }
}

#[cfg(windows)]
impl Drop for BusLock {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

#[cfg(not(windows))]
impl BusLock {
    pub fn new() -> Self {
        Self {}
    }

    pub fn lock(&self, _timeout_ms: u32) -> Option<BusGuard> {
        Some(BusGuard {})
    }
}
