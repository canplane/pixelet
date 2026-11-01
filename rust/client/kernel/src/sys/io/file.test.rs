// pxl-kernel/sys/io/file.test.rs

use super::*;
use crate::sys;

fn read(kept: StorageFile, bytes: &[u8]) -> Option<Vec<u8>> {
    match storage_of(kept, bytes.to_vec()) {
        StorageRead::Bytes(bytes) => Some(bytes),
        StorageRead::None | StorageRead::Unreadable => None,
    }
}

/// What an owner keeps reads back as it wrote it, its slot's format first.
#[test]
fn storage_file_format() {
    let _g = sys::serial();
    for kept in StorageFile::ALL {
        on_emptied(kept);
        let bytes = sealed(kept, &[7, 0, 255]);
        assert_eq!(bytes.first(), Some(&kept.format()));
        assert_eq!(read(kept, &bytes), Some(vec![7, 0, 255]));
        assert_eq!(read(kept, &sealed(kept, &[])), Some(vec![]));
        assert!(matches!(storage_of(kept, Vec::new()), StorageRead::None));
        assert!(!is_refused(kept));
    }
}

/// Another format, or a packet kept with no format before it, is refused
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
        assert_eq!(Path::of(n).as_str(), n.to_string());
    }
    for kept in StorageFile::ALL {
        assert_eq!(kept.path().as_str(), kept.slot().to_string());
        assert_eq!(StorageFile::of_slot(kept.slot()), Some(kept));
    }
}
