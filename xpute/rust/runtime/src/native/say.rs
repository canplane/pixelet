// xpute-runtime/native/say.rs

//! A native core's stop is told on the process's error stream. On Android it
//! goes to the log under the tag `xpute` instead, because an Android app's
//! error stream goes nowhere.

use xpute_core::stop::Stop;

use super::thread::stopped;

/// This is xpute's stop hook for a core linked into its runtime's process
/// (`Form::stop_hook`). It writes one line for a broken invariant or a panic
/// (`say`), and tells the core's thread once the core has ended (`stopped`).
pub fn stop_hook(stop: Stop<'_>) {
    let line = match stop {
        Stop::Bug { errnum, at, a, b } => format!(
            "{errnum:?} at {}:{}:{} ({a}, {b})",
            at.file(),
            at.line(),
            at.column()
        ),
        Stop::Panic(info) => format!("{info}"),
        Stop::End => {
            stopped();
            return;
        }
    };
    say(&line);
}

/// Writes a line to the process's error stream, or to Android's log.
pub fn say(line: &str) {
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
