// pxl-spec/division.spec.rs
//
// GENERATED from spec/spec/division.json (xpute-gen data.rs)
//
// A division's subtype, Overture's, broadest first (pxl-spec division.rs
// `Subtype`): a name is a division and its ancestors, and the client picks
// how deep to read it. The numbers are stored, so a subtype is appended,
// never inserted.

pub mod division_name {
    pub const BYTES: u32 = 128;
}

pub mod division_subtype {
    pub const COUNTRY: u8 = 0;
    pub const DEPENDENCY: u8 = 1;
    pub const MACROREGION: u8 = 2;
    pub const REGION: u8 = 3;
    pub const MACROCOUNTY: u8 = 4;
    pub const COUNTY: u8 = 5;
    pub const LOCALADMIN: u8 = 6;
    pub const LOCALITY: u8 = 7;
    pub const BOROUGH: u8 = 8;
    pub const MACROHOOD: u8 = 9;
    pub const NEIGHBORHOOD: u8 = 10;
    pub const MICROHOOD: u8 = 11;
}
