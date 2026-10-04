// pxl-kernel/door/apply.test.rs

use super::*;
use xpute_core::wire::xtp::PacketWriter;

fn top_z(n: f64) -> Result<u32, Errno> {
    let mut buf = [0u8; 64];
    let len = {
        let mut w = PacketWriter::new(&mut buf, 1);
        w.f64(Some(n));
        w.finish().expect("the buffer holds one number")
    };
    abi::WorldSource::read(&buf[..len as usize]).map(|p| p.top_z).map_err(|e| e.errno)
}

#[test]
fn a_number_the_shell_sends_is_refused_unless_it_is_what_the_field_holds() {
    assert_eq!(top_z(13.0), Ok(13));
    for n in [f64::NAN, f64::INFINITY, -1.0, 2.5, 1e20] {
        assert_eq!(top_z(n), Err(Errno::EINVAL), "{n} is no u32");
    }
}
