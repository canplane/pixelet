// pxl-kernel/sys/io/file.test.rs

use super::*;

fn read(kept: Kept, text: &[u8]) -> Option<Vec<u8>> {
    match kept_of(kept, text) {
        KeptFile::Bytes(bytes) => Some(bytes),
        KeptFile::None | KeptFile::Unreadable => None,
    }
}

/// What an owner keeps reads back as it wrote it, its slot's format first.
#[test]
fn a_kept_file_is_its_format_then_its_owner_s_bytes() {
    let _g = crate::sys::serial();
    for kept in Kept::ALL {
        on_emptied(kept);
        let text = sealed(kept, &[7, 0, 255]);
        assert_eq!(
            BASE64.decode(&text).ok().and_then(|b| b.first().copied()),
            Some(kept.format())
        );
        assert_eq!(read(kept, &text), Some(vec![7, 0, 255]));
        assert_eq!(read(kept, &sealed(kept, &[])), Some(vec![]));
        assert!(matches!(kept_of(kept, b""), KeptFile::None));
        assert!(!is_refused(kept));
    }
}

/// Another format, a file not base64, or the objects' blob as it was kept
/// before it carried its format, is refused until the slot is emptied.
#[test]
fn a_file_of_another_format_is_refused_until_emptied() {
    let _g = crate::sys::serial();
    let foreign = BASE64
        .encode([format::OBJECTS.wrapping_add(1), 1, 2])
        .into_bytes();
    // An XTP packet begins "XTP\0": what the objects kept, alone, before.
    let unformatted = BASE64.encode(b"XTP\0\0\0\0\0").into_bytes();
    for text in [foreign, unformatted, br#"{"ao":"screen"}"#.to_vec()] {
        on_emptied(Kept::Objects);
        assert!(matches!(
            kept_of(Kept::Objects, &text),
            KeptFile::Unreadable
        ));
        assert!(is_refused(Kept::Objects) && !is_refused(Kept::Settings));
    }
    on_emptied(Kept::Objects);
    assert!(!is_refused(Kept::Objects));
}

#[test]
fn an_object_s_path_is_its_number_in_decimal() {
    for n in [0, 1, 9, 10, 702561541869797376, u64::MAX] {
        assert_eq!(Path::of(n).as_str(), n.to_string());
    }
    for kept in Kept::ALL {
        assert_eq!(kept.path().as_str(), kept.slot().to_string());
        assert_eq!(Kept::of_slot(kept.slot()), Some(kept));
    }
}
