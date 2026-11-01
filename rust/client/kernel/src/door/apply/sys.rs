// pxl-kernel/door/apply/sys.rs

//! SYS: the system's: the kernel's start, and the platform's sleep and wake.

use super::State;
use super::{ask_turn, given};
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::geolocation;
use crate::domain::layer::object::repository;
use crate::domain::view::setting::store;
use crate::domain::view::setting::store::settings_open;
use crate::turn;
use crate::turn::world_platform;
use crate::turn::Platform;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::SYS_BOOT => {
            let p = abi::SysBoot::read(given(pkt)?)?;
            super::super::on_host_attached(s);
            store::set_development(p.development);
            turn::set_world_platform(Platform {
                geolocation: p.geolocation,
                orient: device_orientation::OrientationPlatform {
                    supported: p.orient,
                    needs_permission: p.orient_permission,
                    silence_is_loss: p.orient_silence_is_loss,
                },
            });
            // The settings' slot, read from a turn on: what waits on the
            // settings, the profiler and the snapshot's, starts once they are
            // in (turn.rs). The objects are read a shard at a time as the view
            // asks for them (domain/layer/object/repository.rs).
            settings_open();
            // The shell holds the kernel's state from its first render on.
            ask_turn(s)
        }
        Command::SYS_SLEEP => {
            let visible = abi::SysSleep::read(given(pkt)?)?.awake;
            geolocation::geolocation_visibility(visible, world_platform().geolocation);
            if visible {
                device_orientation::orient_visible();
            } else {
                repository::flush();
            }
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}
