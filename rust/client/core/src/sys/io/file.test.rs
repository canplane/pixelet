// pxl-core/sys/io/file.test.rs

use super::*;
use crate::sys::io::demand::Stream;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// Stands in for the client's answer to `open_storage` where there is no
/// runtime: the bytes read whole, or an open that brought none.
pub fn on_storage_answered(answer: Result<&[u8], OpenResult>) {
    match answer {
        Ok(bytes) => set_read(ReadState::Read(storage_of(bytes))),
        Err(opened) => on_storage_opened(0, opened),
    }
}

fn read(bytes: &[u8]) -> Option<Vec<u8>> {
    match storage_of(bytes) {
        StorageRead::Bytes(bytes) => Some(bytes),
        StorageRead::None | StorageRead::Unreadable => None,
    }
}

/// What the settings keep is read without their format byte.
#[test]
fn storage_file_format() {
    crate::on_core(|| {
        on_storage_emptied();
        assert_eq!(read(&[format::SETTINGS, 7, 0, 255]), Some(vec![7, 0, 255]));
        assert_eq!(read(&[format::SETTINGS]), Some(vec![]));
        assert!(matches!(storage_of(&[]), StorageRead::None));
        assert!(!is_storage_refused());
    });
}

/// Another format, or a packet stored with no format before it, is refused
/// until the slot is emptied.
#[test]
fn foreign_format_refused() {
    crate::on_core(|| {
        let foreign = vec![format::SETTINGS.wrapping_add(1), 1, 2];
        // An XTP packet begins "XTP\0".
        let unformatted = b"XTP\0\0\0\0\0".to_vec();
        for bytes in [foreign, unformatted, br#"{"ao":"screen"}"#.to_vec()] {
            on_storage_emptied();
            assert!(matches!(storage_of(&bytes), StorageRead::Unreadable));
            assert!(is_storage_refused());
        }
        on_storage_emptied();
        assert!(!is_storage_refused());
    });
}

/// A file its backend cannot read as bytes is refused, as a file of another
/// format is. Any other failure leaves the file unread but not refused.
#[test]
fn unreadable_bytes_refused() {
    crate::on_core(|| {
        for (errnum, refused) in [(Errno::EBADMSG, true), (Errno::EIO, false)] {
            on_storage_emptied();
            while take_storage().is_some() {}
            on_storage_answered(Err(OpenResult::Failed { errnum, detail: 0 }));
            assert!(matches!(take_storage(), Some(StorageRead::Unreadable)));
            assert_eq!(is_storage_refused(), refused);
        }
        on_storage_emptied();
    });
}

/// The paths the core opens, each built at the length its branch takes.
#[test]
fn path_tokens() {
    crate::on_core(|| {
        let shard = Quadrant::of_key("0123").expect("a tile");
        let (object, top) = (path::OBJECT::ID, path::OBJECT::TOP::ID);
        assert_eq!(
            STORAGE,
            [
                Token::U32(path::STORAGE::ID),
                Token::U32(path::STORAGE::SETTINGS::ID)
            ]
        );
        with_quadrant(path::TERRAIN::ID, shard.id(), |p| {
            assert_eq!(p, [Token::U32(path::TERRAIN::ID), Token::Str("0123")]);
        });
        let top_path = with_path(Stream::object, TOP_ID, |p| {
            assert_eq!(p, [Token::U32(object), Token::U32(top)]);
        });
        assert!(top_path.is_some());
        let leaf = "3".repeat(KEY_BYTES);
        let leaf_id = Quadrant::of_key(&leaf).expect("a leaf").id();
        let leaf_path = with_path(Stream::terrain, leaf_id, |p| p[1] == Token::Str(&leaf));
        assert_eq!(leaf_path, Some(true), "a leaf's whole quadkey");
    });
}

/// A received path is walked as it came to the file it names: the storage
/// file, or an objects' file by its want's id. The edits name none, and nor
/// does a path of another branch, a string that is no quadkey, or a path
/// longer or shorter than its branch.
#[test]
fn path_file() {
    crate::on_core(|| {
        let file_of = |tokens: &[Token]| {
            let mut buf = [0u8; 256];
            let mut w = PacketWriter::new(&mut buf, 1);
            fs::Path::write(&mut w, tokens);
            let n = w.finish().expect("a packet") as usize;
            let f = TreeReader::new(&buf[..n])
                .and_then(|r| r.read_branch())
                .expect("a tree");
            let c = f.at(0).expect("a path");
            of_path(fs::Path::read(&buf[..n], c).expect("a path"))
        };
        let shard = Quadrant::of_key("0123").expect("a tile").id();
        let object = Token::U32(path::OBJECT::ID);
        assert_eq!(file_of(&STORAGE), Some(File::Storage));
        with_quadrant(path::OBJECT::ID, shard, |p| {
            assert_eq!(file_of(p), Some(File::Objects(shard)));
        });
        with_quadrant(path::OBJECT::ID, TOP_ID, |p| {
            assert_eq!(file_of(p), Some(File::Objects(TOP_ID)));
        });
        let bad: [&[Token]; 5] = [
            &[object, Token::U32(path::OBJECT::EDIT::ID)],
            &[Token::U32(path::TERRAIN::ID), Token::Str("0123")],
            &[object, Token::Str("0124")],
            &[object, Token::Str("0123"), Token::U32(0)],
            &[object],
        ];
        for tokens in bad {
            assert_eq!(file_of(tokens), None);
        }
    });
}
