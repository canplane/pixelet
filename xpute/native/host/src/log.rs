// xpute-native-host/log.rs

//! A line of the host's, or of what it serves, to the process's error
//! stream, or to Android's log under the tag `xpute`, because an Android
//! app's error stream goes nowhere.

/// Writes `line`.
pub fn log(line: &str) {
    #[cfg(target_os = "android")]
    {
        let line = std::ffi::CString::new(line.replace('\0', " ")).unwrap_or_default();
        // SAFETY: two strings, ours for the call.
        unsafe {
            ndk_sys::__android_log_write(
                ndk_sys::android_LogPriority::ANDROID_LOG_ERROR.0 as i32,
                c"xpute".as_ptr(),
                line.as_ptr(),
            )
        };
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("{line}");
}
