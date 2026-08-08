//! Wake timer so Windows can resume from sleep to run a scheduled action.

#[cfg(windows)]
use parking_lot::Mutex;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    CancelWaitableTimer, CreateWaitableTimerW, SetWaitableTimer,
};

/// Stores a waitable timer handle. HANDLE is not Send; we mark this safe
/// because access is serialized via the mutex and we never share the handle.
pub struct WakeTimer {
    #[cfg(windows)]
    handle: Mutex<Option<isize>>,
}

unsafe impl Send for WakeTimer {}
unsafe impl Sync for WakeTimer {}

impl WakeTimer {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            handle: Mutex::new(None),
        }
    }

    pub fn clear(&self) {
        #[cfg(windows)]
        {
            let mut guard = self.handle.lock();
            if let Some(raw) = guard.take() {
                let handle = HANDLE(raw as *mut std::ffi::c_void);
                unsafe {
                    let _ = CancelWaitableTimer(handle);
                    let _ = CloseHandle(handle);
                }
            }
        }
    }

    /// Arm a wake at absolute unix timestamp (seconds). No-op if `unix` is in the past.
    pub fn arm_at_unix(&self, unix: i64) {
        #[cfg(windows)]
        {
            use chrono::Utc;
            let now = Utc::now().timestamp();
            let delta = unix - now;
            if delta <= 0 {
                self.clear();
                return;
            }

            // Relative due time: negative 100-ns intervals
            let due: i64 = -delta.saturating_mul(10_000_000);

            self.clear();
            unsafe {
                let Ok(handle) = CreateWaitableTimerW(None, true, None) else {
                    return;
                };
                if SetWaitableTimer(handle, &due, 0, None, None, true).is_ok() {
                    *self.handle.lock() = Some(handle.0 as isize);
                } else {
                    let _ = CloseHandle(handle);
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = unix;
        }
    }
}

impl Default for WakeTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WakeTimer {
    fn drop(&mut self) {
        self.clear();
    }
}
