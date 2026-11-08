// pxl-core/sys/io/file.test.rs

use super::*;
use crate::sys;

/// The client's answer to `open_storage` where there is no runtime: `bytes`
/// read whole, or an open that came to none.
pub fn on_storage_answered(answer: Result<&[u8], Opened>) {
    match answer {
        Ok(bytes) => set_read(ReadState::Read(storage_of(bytes.to_vec()))),
        Err(opened) => on_storage_opened(0, opened),
    }
}

fn read(bytes: &[u8]) -> Option<Vec<u8>> {
    match storage_of(bytes.to_vec()) {
        StorageRead::Bytes(bytes) => Some(bytes),
        StorageRead::None | StorageRead::Unreadable => None,
    }
}

/// What the settings keep is read past their format byte.
#[test]
fn storage_file_format() {
    let _g = sys::serial();
    on_storage_emptied();
    assert_eq!(read(&[format::SETTINGS, 7, 0, 255]), Some(vec![7, 0, 255]));
    assert_eq!(read(&[format::SETTINGS]), Some(vec![]));
    assert!(matches!(storage_of(Vec::new()), StorageRead::None));
    assert!(!is_storage_refused());
}

/// Another format, or a packet stored with no format before it, is refused
/// until the slot is emptied.
#[test]
fn foreign_format_refused() {
    let _g = sys::serial();
    let foreign = vec![format::SETTINGS.wrapping_add(1), 1, 2];
    // An XTP packet begins "XTP\0".
    let unformatted = b"XTP\0\0\0\0\0".to_vec();
    for bytes in [foreign, unformatted, br#"{"ao":"screen"}"#.to_vec()] {
        on_storage_emptied();
        assert!(matches!(storage_of(bytes), StorageRead::Unreadable));
        assert!(is_storage_refused());
    }
    on_storage_emptied();
    assert!(!is_storage_refused());
}

/// A file its backend cannot read as bytes is refused as one of another
/// format; any other failure leaves it unread, not refused.
#[test]
fn unreadable_bytes_refused() {
    let _g = sys::serial();
    for (errnum, refused) in [(Errno::EBADMSG, true), (Errno::EIO, false)] {
        on_storage_emptied();
        while take_storage().is_some() {}
        on_storage_answered(Err(Opened::Failed { errnum, detail: 0 }));
        assert!(matches!(take_storage(), Some(StorageRead::Unreadable)));
        assert_eq!(is_storage_refused(), refused);
    }
    on_storage_emptied();
}

#[test]
fn object_path_decimal() {
    for n in [0, 1, 9, 10, 702561541869797376, u64::MAX] {
        assert_eq!(Path::of_id(n).as_str(), n.to_string());
    }
    assert_eq!(storage_path().as_str(), storage::SETTINGS.to_string());
}

/// A key's path and back: a shard's quadkey and the top's own name.
#[test]
fn path_of_key() {
    let shard = Quadrant::of_key("0123").expect("a tile");
    assert_eq!(Path::of_quadrant(shard.id()).as_str(), "0123");
    assert_eq!(id_of_path("0123"), Some(shard.id()));
    assert_eq!(Path::of_quadrant(TOP_ID).as_str(), object::TOP);
    assert_eq!(id_of_path(object::TOP), Some(TOP_ID));
    assert_eq!(id_of_path(object::EDITS), None);
    assert_eq!(Path::of_id(u64::MAX).as_str(), "18446744073709551615");
}
