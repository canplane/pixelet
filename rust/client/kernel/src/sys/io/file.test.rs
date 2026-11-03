// pxl-kernel/sys/io/file.test.rs

use super::*;
use crate::sys;

fn read(file: StorageFile, bytes: &[u8]) -> Option<Vec<u8>> {
    match storage_of(file, bytes.to_vec()) {
        StorageRead::Bytes(bytes) => Some(bytes),
        StorageRead::None | StorageRead::Unreadable => None,
    }
}

/// What an owner keeps reads back as it wrote it, its slot's format first.
#[test]
fn storage_file_format() {
    let _g = sys::serial();
    for file in StorageFile::ALL {
        on_emptied(file);
        let bytes = seal(file, &[7, 0, 255]);
        assert_eq!(bytes.first(), Some(&file.format()));
        assert_eq!(read(file, &bytes), Some(vec![7, 0, 255]));
        assert_eq!(read(file, &seal(file, &[])), Some(vec![]));
        assert!(matches!(storage_of(file, Vec::new()), StorageRead::None));
        assert!(!is_refused(file));
    }
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
        on_emptied(StorageFile::Settings);
        assert!(matches!(
            storage_of(StorageFile::Settings, bytes),
            StorageRead::Unreadable
        ));
        assert!(is_refused(StorageFile::Settings));
    }
    on_emptied(StorageFile::Settings);
    assert!(!is_refused(StorageFile::Settings));
}

/// A file its backend cannot read as bytes is refused as one of another
/// format; any other failure leaves it unread, not refused.
#[test]
fn unreadable_bytes_refused() {
    let _g = sys::serial();
    for (errnum, refused) in [(Errno::EBADMSG, true), (Errno::EIO, false)] {
        on_emptied(StorageFile::Settings);
        while take_storage(StorageFile::Settings).is_some() {}
        on_storage_answered(
            StorageFile::Settings,
            Err(Opened::Failed { errnum, detail: 0 }),
        );
        assert!(matches!(
            take_storage(StorageFile::Settings),
            Some(StorageRead::Unreadable)
        ));
        assert_eq!(is_refused(StorageFile::Settings), refused);
    }
    on_emptied(StorageFile::Settings);
}

#[test]
fn object_path_decimal() {
    for n in [0, 1, 9, 10, 702561541869797376, u64::MAX] {
        assert_eq!(Path::of_id(n).as_str(), n.to_string());
    }
    for file in StorageFile::ALL {
        assert_eq!(file.path().as_str(), file.slot().to_string());
        assert_eq!(StorageFile::of_slot(file.slot()), Some(file));
    }
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
