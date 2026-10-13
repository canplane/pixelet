//! The MSL Naga 30 writes for the generated loop shape (seed.rs's case 3),
//! to tell Naga's Metal backend from Metal's own compiler.

fn main() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bin/seed.rs")).unwrap();
    let wgsl = &src[src.find("r#\"").unwrap() + 3..src.find("\"#;").unwrap()];
    let module = naga::front::wgsl::parse_str(wgsl).expect("parse");
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).expect("valid");
    let options = naga::back::msl::Options { lang_version: (2, 4), ..Default::default() };
    let pipeline = naga::back::msl::PipelineOptions::default();
    let (msl, _) = naga::back::msl::write_string(&module, &info, &options, &pipeline).expect("msl");
    let start = msl.find("phi_749_ = 0u").unwrap_or(0);
    println!("{}", &msl[start..(start + 2200).min(msl.len())]);
}
