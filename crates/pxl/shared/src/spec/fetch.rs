// pxl_shared/spec/fetch.rs
// Why a fetch failed, as a number; the words are the shell's.

/// Why a demand the scheduler started did not deliver.
///
/// The shell runs the transport and is the one that catches an exception, so
/// it is the one that knows which of these happened; what crosses is the
/// number. The prose a browser writes for a failed request ("TypeError:
/// Failed to fetch") is the shell's own, and there is nothing the kernel can
/// do with it that it cannot do with this — it neither retries on it nor
/// routes by it, it only tells one failure from another, which a number does.
///
/// An HTTP refusal keeps its status beside this rather than inside it: the
/// status is already a number the shell sends, and folding it in would make
/// one reason per code.
///
/// The number is the name here, as in cmd.rs and message.rs: an old shell
/// reads a number a new kernel sends, so a number that has meant something
/// must never come to mean another thing. Retire one by leaving the gap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum FetchFail {
    /// It did not fail.
    #[default]
    None = 0,

    // ---- 0x01 the request never came back ----
    /// The transport itself failed — offline, DNS, CORS, a refused
    /// connection. `TypeError` is what a browser raises for all of them.
    Network = 0x0101,
    /// The shell raised something else. Its own words say what; the kernel
    /// only needs to count it apart from the rest.
    Shell = 0x0102,

    // ---- 0x02 a response arrived and was not usable ----
    /// A status the reader would not take. `status` says which.
    Status = 0x0201,
    /// The kernel had no room for the body it was offered.
    NoRoom = 0x0202,

    // ---- 0x03 the bytes arrived and did not hold up ----
    /// The body did not decode as its format.
    Decode = 0x0301,
    /// The key named no page.
    BadSubject = 0x0302,
    /// The key named a category nothing serves.
    BadCategory = 0x0303,
}

impl FetchFail {
    /// A short name for a report to print. The words a person reads for a
    /// failure are the shell's; this is the one the kernel's own dev report
    /// needs, and it is a `&'static str` in the module's rodata rather than
    /// anything on its heap.
    pub fn as_str(self) -> &'static str {
        match self {
            FetchFail::None => "none",
            FetchFail::Network => "network",
            FetchFail::Shell => "shell",
            FetchFail::Status => "status",
            FetchFail::NoRoom => "no_room",
            FetchFail::Decode => "decode",
            FetchFail::BadSubject => "bad_subject",
            FetchFail::BadCategory => "bad_category",
        }
    }
}
