use crate::MiMalloc;
#[cfg(not(feature = "v2"))]
use core::ffi::{c_char, CStr};
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::{c_int, c_void},
};

impl MiMalloc {
    /// Get the mimalloc version.
    ///
    /// For mimalloc version 1.8.6, this will return 186.
    pub fn version(&self) -> u32 {
        unsafe { ffi::mi_version() as u32 }
    }

    /// Return the amount of available bytes in a memory block.
    ///
    /// # Safety
    /// `ptr` must point to a memory block allocated by mimalloc, or be null.
    #[inline]
    pub unsafe fn usable_size(&self, ptr: *const u8) -> usize {
        ffi::mi_usable_size(ptr as *const c_void)
    }

    /// Extract a string containing the JSON statistics for the whole process
    ///
    /// Allocates (using mimalloc itself) to store the JSON structure.
    #[cfg(not(feature = "v2"))]
    pub fn stats_json() -> Result<StatsJson, &'static str> {
        unsafe {
            let buf = ffi::mi_stats_get_json(0, core::ptr::null::<c_char>() as *mut _);
            if let Some(inner) = core::ptr::NonNull::new(buf) {
                Ok(StatsJson { inner })
            } else {
                Err("failed to call mi_stats_get_json")
            }
        }
    }

    /// Register a callback function that is called on mimalloc errors.
    ///
    /// ## Note
    ///
    /// The function data is **leaked**, so do not call this function too many times over the life of the program.
    pub fn register_error<F>(f: F)
    where
        F: Fn(ErrorCode) + Send + Sync,
    {
        let layout = Layout::new::<F>();
        let alloc = MiMalloc;
        let arg: *mut F = unsafe { alloc.alloc(layout).cast() };
        unsafe { arg.write(f) };

        unsafe { ffi::mi_register_error(Some(call_error_fn::<F>), arg.cast()) };
    }

    /// Reset the callback function to the default.
    pub fn reset_error() {
        unsafe {
            ffi::mi_register_error(None, core::ptr::null_mut());
        }
    }

    /// Register a callback function that is called on mimalloc messages print.
    ///
    /// ## Note
    ///
    /// The function data is **leaked**, so do not call this function too many times over the life of the program.
    pub fn register_output<F>(f: F)
    where
        F: Fn(&CStr),
    {
        let layout = Layout::new::<F>();
        let alloc = MiMalloc;
        let arg: *mut F = unsafe { alloc.alloc(layout).cast() };
        unsafe { arg.write(f) };

        unsafe {
            ffi::mi_register_output(Some(call_out_fn::<F>), arg.cast());
        }
    }

    /// Reset the callback function for printing messages to the default.
    pub fn reset_output() {
        unsafe {
            ffi::mi_register_output(None, core::ptr::null_mut());
        }
    }
}

unsafe extern "C" fn call_error_fn<F>(code: c_int, arg: *mut c_void)
where
    F: Fn(ErrorCode),
{
    let error_code = ErrorCode::from_code(code);
    let f: *mut F = arg.cast();
    let f = f.as_ref().unwrap();
    f(error_code);
}

unsafe extern "C" fn call_out_fn<F>(msg: *const c_char, arg: *mut c_void)
where
    F: Fn(&CStr),
{
    let msg = CStr::from_ptr(msg);
    let f: *mut F = arg.cast();
    let f = f.as_ref().unwrap();
    f(msg);
}

pub enum ErrorCode {
    DoubleFree,
    CorruptedFreeListOrMetadata,
    OutOfMemory,
    TooLargeRequest,
    InvalidPointer,
    UnexpectedErrorCode(i64),
}

impl ErrorCode {
    pub fn from_code(code: c_int) -> Self {
        match code {
            11 => Self::DoubleFree,
            12 => Self::OutOfMemory,
            14 => Self::CorruptedFreeListOrMetadata,
            22 => Self::InvalidPointer,
            75 => Self::TooLargeRequest,
            code => Self::UnexpectedErrorCode(code as _),
        }
    }
}

#[cfg(not(feature = "v2"))]
/// Wrapper around the output of `MiMalloc::stats_json()`
///
/// Derefs to a CStr
pub struct StatsJson {
    inner: core::ptr::NonNull<c_char>,
}

#[cfg(not(feature = "v2"))]
impl core::ops::Deref for StatsJson {
    type Target = CStr;

    fn deref(&self) -> &Self::Target {
        unsafe {
            let cstr = CStr::from_ptr(self.inner.as_ptr());
            &cstr
        }
    }
}

#[cfg(not(feature = "v2"))]
impl Drop for StatsJson {
    fn drop(&mut self) {
        unsafe { ffi::mi_free(self.inner.as_ptr() as _) }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::alloc::GlobalAlloc;
    use core::alloc::Layout;

    #[test]
    fn it_gets_version() {
        let version = MiMalloc.version();
        assert!(version != 0);
    }

    #[test]
    fn it_checks_usable_size() {
        unsafe {
            let layout = Layout::from_size_align(8, 8).unwrap();
            let alloc = MiMalloc;

            let ptr = alloc.alloc(layout);
            let usable_size = alloc.usable_size(ptr);
            alloc.dealloc(ptr, layout);
            assert!(usable_size >= 8);
        }
    }

    #[test]
    #[cfg(not(feature = "v2"))]
    fn test_stats_json() {
        let stats = MiMalloc::stats_json().expect("should get stats");
        let slice = stats.to_str().expect("should be valid UTF-8");
        assert_eq!(slice.chars().next(), Some('{'));
        assert!(stats.count_bytes() > 1);
    }
}
