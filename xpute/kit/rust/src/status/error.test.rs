// xpute-kit/status/error.test.rs

use super::*;
use crate::status::msg::MSG_CAP;

/// An error is placed where it is made; its message past the cap is cut at a
/// character boundary, and an empty one is none.
#[test]
fn error_is_placed_and_its_msg_cut() {
    let e = XputeError::with_msg(Errno::EIO, "no café");
    assert_eq!((&*e.file, e.line), (file!(), line!() - 1));
    assert_eq!(e.msg.as_deref(), Some("no café"));
    let long = format!("x{}", "é".repeat(MSG_CAP as usize / 2));
    let e = XputeError::with_msg(Errno::EIO, long.as_str());
    assert_eq!(e.msg.as_deref(), Some(&long[..MSG_CAP as usize - 1]));
    assert_eq!(XputeError::with_msg(Errno::EIO, "").msg, None);
    let came = XputeError::at(Errno::EIO, format!("{long}.rs"), 7, Some(long.clone()));
    assert_eq!(&*came.file, &long[..MSG_CAP as usize - 1]);
    assert_eq!(came.msg.as_deref(), Some(&long[..MSG_CAP as usize - 1]));
    assert_eq!(
        XputeError::at(Errno::EIO, String::new(), 7, Some(String::new())).msg,
        None
    );
    let e: XputeError = Errno::EIO.into();
    assert_eq!((&*e.file, e.line, e.msg), (file!(), line!() - 1, None));
}
