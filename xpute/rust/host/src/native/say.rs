// xpute-host/native/say.rs

//! Where a native guest's stop is told: the process's error stream, or
//! Android's log under the tag `xpute`, since an Android app's error stream
//! goes nowhere.

use xpute_guest::stop::Stop;

use super::thread::stopped;

/// xpute's stop hook for a guest linked into its host's process
/// (`BootInfo::stop_hook`): a broken invariant's or a panic's line said
/// (`say`), and the guest's end told its thread (`stopped`).
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

/// A line to the process's error stream, or to Android's log.
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
