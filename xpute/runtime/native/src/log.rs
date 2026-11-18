// xpute-runtime-native/log.rs

//! The runtime's writer, the one place native xpute writes to the platform's
//! output: a log record its host hands it (xpute-host runtime.rs `log`) or its
//! UI does (lib.rs `log`), and the stop and the trap it tells, each as
//! `[context] message`, to Android's log under the tag `xpute` at the priority
//! of its level, because an Android app's error stream goes nowhere, and to the
//! process's error stream elsewhere, which has no levels.

use xpute_host::abi::log::Level;

/// Android's priority of each level, in `Level`'s order.
#[cfg(target_os = "android")]
const PRIORITY: [ndk_sys::android_LogPriority; 6] = [
    ndk_sys::android_LogPriority::ANDROID_LOG_VERBOSE,
    ndk_sys::android_LogPriority::ANDROID_LOG_DEBUG,
    ndk_sys::android_LogPriority::ANDROID_LOG_INFO,
    ndk_sys::android_LogPriority::ANDROID_LOG_WARN,
    ndk_sys::android_LogPriority::ANDROID_LOG_ERROR,
    ndk_sys::android_LogPriority::ANDROID_LOG_FATAL,
];

/// Writes `msg` at `level`, about `context`.
pub(crate) fn log(level: Level, context: &str, msg: &str) {
    let line = format!("[{context}] {msg}");
    #[cfg(target_os = "android")]
    {
        let line = std::ffi::CString::new(line.replace('\0', " ")).unwrap_or_default();
        // SAFETY: two strings, ours for the call.
        unsafe {
            ndk_sys::__android_log_write(
                PRIORITY[level as usize].0 as i32,
                c"xpute".as_ptr(),
                line.as_ptr(),
            )
        };
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = level;
        eprintln!("{line}");
    }
}
