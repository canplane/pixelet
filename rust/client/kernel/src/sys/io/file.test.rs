// pxl-kernel/sys/io/file.test.rs

use super::*;
use crate::sys;

fn read(kept: StorageFile, text: &[u8]) -> Option<Vec<u8>> {
    match storage_of(kept, text) {
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
        let text = sealed(kept, &[7, 0, 255]);
        assert_eq!(
            BASE64.decode(&text).ok().and_then(|b| b.first().copied()),
            Some(kept.format())
        );
        assert_eq!(read(kept, &text), Some(vec![7, 0, 255]));
        assert_eq!(read(kept, &sealed(kept, &[])), Some(vec![]));
        assert!(matches!(storage_of(kept, b""), StorageRead::None));
        assert!(!is_refused(kept));
    }
}

/// Another format, a file not base64, or a packet kept with no format
/// before it, is refused until the slot is emptied.
#[test]
fn foreign_format_refused() {
    let _g = sys::serial();
    let foreign = BASE64
        .encode([format::SETTINGS.wrapping_add(1), 1, 2])
        .into_bytes();
    // An XTP packet begins "XTP\0".
    let unformatted = BASE64.encode(b"XTP\0\0\0\0\0").into_bytes();
    for text in [foreign, unformatted, br#"{"ao":"screen"}"#.to_vec()] {
        on_emptied(StorageFile::Settings);
        assert!(matches!(
            storage_of(StorageFile::Settings, &text),
            StorageRead::Unreadable
        ));
        assert!(is_refused(StorageFile::Settings));
    }
    on_emptied(StorageFile::Settings);
    assert!(!is_refused(StorageFile::Settings));
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
