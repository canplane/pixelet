// xpute-runtime/native/serve.test.rs

use std::collections::HashMap;

use super::*;
use xpute_core::abi::fs::FsWrite;
use xpute_core::io::fs::Token;
use xpute_kit::golden::Golden;
use xpute_kit::wire::xtp::TreeReader;

const TRACE: &str = include_str!("../../../../spec/conformance/fs/file.tsv");

/// Descriptors no trace sends, so that each completion read here is this
/// test's.
const OPENING: u32 = 0x7702;
const CLOSING: u32 = 0x7703;
const UNCLOSED: u32 = 0x7704;
const READ: u32 = 0x7705;

/// Where a trace's reads and writes name the core's memory.
const SCRATCH: u32 = 1 << 15;

/// The tests here take this before they touch `SHARED` or `QUEUE`, so that
/// no two of them read or replace the other's state.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A memory of its own for a test, which outlives every use the test makes.
fn mem_of(buf: &mut [u8]) -> Memory {
    // SAFETY: the caller's buffer outlives the memory's use.
    unsafe { Memory::new(buf.as_mut_ptr() as usize, buf.len()) }
}

fn close_of(fd: u32) -> Down {
    frame_of(fd, FsCommand::FS_CLOSE, Vec::new(), &[])
}

fn frame_of(tag: u32, cmd: FsCommand, pkt: Vec<u8>, named: &[u8]) -> Down {
    let ackreq = matches!(cmd, FsCommand::FS_OPEN | FsCommand::FS_CLOSE);
    Down {
        tag,
        cmd: cmd as u32,
        flags: if ackreq { frame::ACKREQ } else { 0 },
        result: 0,
        pkt,
        named: named.to_vec(),
    }
}

/// The completions queued for the core under `tag`, taken, as their command
/// and result.
fn completed(tag: u32) -> Vec<(u32, i32)> {
    let mut q = queue();
    let (mine, rest): (Vec<Up>, Vec<Up>) = q.up.drain(..).partition(|u| u.tag == tag);
    q.up.extend(rest);
    mine.into_iter()
        .map(|u| {
            assert_ne!(u.flags & frame::RES, 0);
            (u.cmd, u.result)
        })
        .collect()
}

/// What `down` holds for the UI: each frame's tag, command, packet and the
/// memory it named.
fn frames(down: &[u8]) -> Vec<(u32, u32, Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    let mut at = 0;
    let align = ALIGN as usize;
    while at < down.len() {
        let word = |k: u32| {
            let k = at + 4 * k as usize;
            u32::from_le_bytes(down[k..k + 4].try_into().unwrap())
        };
        let (tag, cmd) = (word(batch::TAG), word(batch::CMD));
        let (pkt_len, named_len) = (word(batch::PKT) as usize, word(batch::NAMED) as usize);
        at += 4 * HEADER_WORDS as usize;
        let pkt = down[at..at + pkt_len].to_vec();
        at = (at + pkt_len).next_multiple_of(align);
        out.push((tag, cmd, pkt, down[at..at + named_len].to_vec()));
        at = (at + named_len).next_multiple_of(align);
    }
    out
}

/// What `down` holds for the UI: each frame's command and the memory it named.
fn heard(down: &[u8]) -> Vec<(u32, Vec<u8>)> {
    frames(down)
        .into_iter()
        .map(|(_, cmd, _, named)| (cmd, named))
        .collect()
}

/// A path as the trace writes it, a JSON array of its tokens, as the key its
/// file is kept under.
fn trace_key(s: &str) -> String {
    let inner = s.trim().trim_start_matches('[').trim_end_matches(']');
    let tokens: Vec<String> = inner
        .split(',')
        .map(|t| t.trim().trim_matches('"').to_string())
        .collect();
    tokens.join("/")
}

/// The key of the path a packet carries.
fn key_of(path: Path) -> String {
    let tokens: Vec<String> = path
        .iter()
        .map(|t| match t {
            Token::U32(n) => n.to_string(),
            Token::Str(s) => s.to_string(),
        })
        .collect();
    tokens.join("/")
}

/// The path COMMIT's packet carries, alone.
fn commit_path(pkt: &[u8]) -> String {
    let f = TreeReader::new(pkt).unwrap().read_branch().unwrap();
    key_of(Path::read(pkt, f.at(0).unwrap()).unwrap())
}

fn errno(name: &str) -> i32 {
    (1..4096)
        .filter_map(|n| Errno::of(-n))
        .find(|e| format!("{e:?}") == name)
        .unwrap_or_else(|| panic!("no errno {name}")) as i32
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).unwrap())
        .collect()
}

fn number(s: &str) -> u32 {
    match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16).unwrap(),
        None => s.parse().unwrap(),
    }
}

/// Runs `scenario`: a step writes a frame as a core's turn would, `turn`
/// serves them as this thread does, and the test answers what reaches the UI
/// as a program's binding would.
fn run(g: &Golden, buf: &mut [u8], scenario: &str) {
    let mem = mem_of(buf);
    *lock() = Some(Shared {
        fs: true,
        ..fresh()
    });
    queue().up.clear();
    let mut turn: Vec<Down> = Vec::new();
    let mut files: HashMap<String, &str> = HashMap::new();
    let mut refusals: HashMap<String, i32> = HashMap::new();
    let mut waiting: HashMap<String, u32> = HashMap::new();
    let mut commits: HashMap<String, Vec<u8>> = HashMap::new();
    let mut steps = Vec::new();
    g.each(&format!("{scenario}.step"), |k| steps.push(g.s(k)));
    let deliver = |fd: u32, b: &str| match b {
        "ENOENT" => fs_complete_open(fd, errno("ENOENT"), 0, Vec::new()),
        b => fs_complete_open(fd, (b.len() / 2) as i32, 0, hex(b)),
    };
    for step in steps {
        let at = format!("{scenario}: {step}");
        let a: Vec<&str> = step.split(' ').collect();
        match a[0] {
            "file" => _ = files.insert(trace_key(a[1]), a[2]),
            "refuse" => _ = refusals.insert(trace_key(a[1]), errno(a[2])),
            "open" => {
                let flags = match a[3] {
                    "O_RDONLY" => flag::O_RDONLY,
                    "O_WRONLY" => flag::O_WRONLY,
                    n => number(n),
                };
                let tokens: Vec<String> = trace_key(a[2]).split('/').map(String::from).collect();
                let path: Vec<Token> = tokens
                    .iter()
                    .map(|t| t.parse().map_or(Token::Str(t), Token::U32))
                    .collect();
                let open = FsOpen { path: &path, flags };
                let pkt = pkt(FsOpen::FIELDS, |w| open.write(w));
                turn.push(frame_of(number(a[1]), FsCommand::FS_OPEN, pkt, &[]));
            }
            "read" => {
                let at = a.get(3).map_or(SCRATCH, |s| number(s));
                let read = FsRead {
                    at,
                    len: number(a[2]),
                };
                let pkt = pkt(FsRead::FIELDS, |w| read.write(w));
                turn.push(frame_of(number(a[1]), FsCommand::FS_READ, pkt, &[]));
            }
            "write" => {
                let bytes = a[2].as_bytes();
                let write = FsWrite {
                    at: SCRATCH,
                    len: bytes.len() as u32,
                };
                let pkt = pkt(FsWrite::FIELDS, |w| write.write(w));
                turn.push(frame_of(number(a[1]), FsCommand::FS_WRITE, pkt, bytes));
            }
            "close" => turn.push(close_of(number(a[1]))),
            "turn" => {
                with(|s| turn.drain(..).for_each(|f| take_frame(s, mem, f)));
                for (tag, cmd, pkt, named) in frames(&take()) {
                    if cmd == FsCommand::FS_OPEN as u32 {
                        let key = key_of(FsOpen::parts(&pkt).unwrap().0);
                        match files.get(&key).copied() {
                            Some("later") => _ = waiting.insert(key, tag),
                            Some(b) => deliver(tag, b),
                            None => deliver(tag, "ENOENT"),
                        }
                    } else if cmd == COMMIT {
                        let key = commit_path(&pkt);
                        let res = match refusals.get(&key) {
                            Some(&err) => err,
                            None => {
                                commits.insert(key, named);
                                0
                            }
                        };
                        post(tag, FsCommand::FS_CLOSE as u32, frame::RES, res, &[]);
                    }
                }
            }
            "deliver" => {
                if let Some(fd) = waiting.remove(&trace_key(a[1])) {
                    deliver(fd, a[2]);
                }
            }
            "expect" => match a[1] {
                "opened" | "closed" => {
                    let cmd = if a[1] == "opened" {
                        FsCommand::FS_OPEN
                    } else {
                        FsCommand::FS_CLOSE
                    };
                    let res = a[3].parse().unwrap_or_else(|_| errno(a[3]));
                    let up = queue().up.pop_front().map(|u| (u.cmd, u.tag, u.result));
                    assert_eq!(up, Some((cmd as u32, number(a[2]), res)), "{at}");
                }
                "quiet" => assert!(queue().up.is_empty(), "{at}"),
                "read" => {
                    let want = hex(a[2]);
                    // SAFETY: the test's memory, which no frame is writing.
                    let got = unsafe {
                        core::slice::from_raw_parts(mem.at::<u8>(SCRATCH as usize), want.len())
                    };
                    assert_eq!(got, want, "{at}");
                }
                "commit" => {
                    let got = commits.get(&trace_key(a[2]));
                    assert_eq!(got.map(Vec::as_slice), Some(a[3].as_bytes()), "{at}");
                }
                "unwritten" => assert!(!commits.contains_key(&trace_key(a[2])), "{at}"),
                _ => panic!("{at}: no such expectation"),
            },
            _ => panic!("{at}: no such step"),
        }
    }
    *lock() = None;
}

/// The device's runtime keeps the file device's trace, as the page's does
/// (@xpute/runtime io/fs.test.ts).
#[test]
fn file_device_keeps_the_trace() {
    let _serial = serial();
    let g = Golden::load(TRACE);
    for scenario in g.s("scenarios").split(' ') {
        let mut buf = vec![0u8; 1 << 16];
        run(&g, &mut buf, scenario);
    }
}

/// A reload completes an open and a commit that the program left with
/// ECANCELED. A file read and a written file not yet closed go on, the
/// written one committed by the next program, and no frame of the file device
/// goes to that program from the last.
#[test]
fn reload_cancels_what_the_program_left() {
    let _serial = serial();
    let mut s = fresh();
    s.files.insert(
        OPENING,
        File::Read {
            bytes: None,
            err: 0,
        },
    );
    let written = |closing| File::Write {
        path: Vec::new(),
        bytes: b"ab".to_vec(),
        closing,
        err: 0,
    };
    s.files.insert(CLOSING, written(true));
    s.files.insert(UNCLOSED, written(false));
    s.files.insert(
        READ,
        File::Read {
            bytes: Some(vec![1]),
            err: 0,
        },
    );
    let close = FsCommand::FS_CLOSE as u32;
    down(&mut s, OPENING, FsCommand::FS_OPEN as u32, 0, 0, &[], &[]);
    down(&mut s, 9, 0x1001, 0, 0, &[1, 2], &[3]);
    down(&mut s, CLOSING, close, 0, 0, &[], &[]);
    let kept = without_fs(&s.down);
    assert_eq!(
        heard(&kept),
        [(0x1001, vec![3])],
        "the program's own frame alone"
    );
    let mut files = cancel(s.files);
    let canceled = Errno::ECANCELED as i32;
    assert_eq!(completed(OPENING), [(FsCommand::FS_OPEN as u32, canceled)]);
    assert_eq!(completed(CLOSING), [(close, canceled)]);
    assert!(completed(UNCLOSED).is_empty() && completed(READ).is_empty());
    let mut s = Shared {
        fs: true,
        files: core::mem::take(&mut files),
        ..fresh()
    };
    let mut buf = vec![0u8; 64];
    let mem = mem_of(&mut buf);
    take_frame(&mut s, mem, close_of(UNCLOSED));
    assert_eq!(
        heard(&s.down),
        [(COMMIT, b"ab".to_vec())],
        "the next program commits it"
    );
    take_frame(&mut s, mem, close_of(READ));
    assert_eq!(completed(READ), [(close, 0)], "a file read goes on");
}
