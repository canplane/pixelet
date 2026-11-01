// pxl-kernel/door/apply/sensor.rs

//! SENSOR: the device sensors': their readings, failures and permissions, and the location asked for.

use super::State;
use super::{ask_turn, bearing, given, on_earth, refuse_unless};
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::geolocation;
use crate::turn::world_platform;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::SENSOR_POS => {
            let p = abi::SensorPos::read(given(pkt)?)?;
            refuse_unless(
                on_earth(p.lon, p.lat) && p.accuracy.is_none_or(|a| a >= 0.0) && bearing(p.course),
                Errno::EINVAL,
            )?;
            // An accuracy the device does not give is no accuracy at all.
            geolocation::geolocation_fix(
                p.lon,
                p.lat,
                p.alt,
                p.accuracy.unwrap_or(f64::INFINITY),
                p.course,
            );
            Ok(Some(0))
        }
        Command::SENSOR_ERR => {
            let p = abi::SensorErr::read(given(pkt)?)?;
            match p.sensor {
                // A failure the platform named no cause for is Other's.
                abi::Sensor::Geolocation => {
                    geolocation::geolocation_err(
                        p.code.unwrap_or(abi::GeolocationErrorCode::Other),
                    );
                }
                abi::Sensor::Orientation => device_orientation::on_orient_lost(),
            }
            Ok(Some(0))
        }
        Command::SENSOR_ORIENT => {
            let p = abi::SensorOrient::read(given(pkt)?)?;
            // DeviceOrientationEvent's ranges, closed: a reading at an end is still one.
            let within = |v: Option<f64>, half: f64| v.is_none_or(|v| (-half..=half).contains(&v));
            refuse_unless(
                bearing(p.webkit_compass_heading)
                    && bearing(p.alpha)
                    && within(p.beta, 180.0)
                    && within(p.gamma, 90.0),
                Errno::EINVAL,
            )?;
            device_orientation::orient_event(p.webkit_compass_heading, p.alpha, p.beta, p.gamma);
            Ok(Some(0))
        }
        Command::SENSOR_PERMISSION => {
            let p = abi::SensorPermission::read(given(pkt)?)?;
            match p.sensor {
                abi::Sensor::Geolocation => {
                    if p.answer == abi::PermissionAnswer::Granted {
                        geolocation::on_geolocation_permission_granted(
                            world_platform().geolocation,
                        );
                    }
                }
                abi::Sensor::Orientation => {
                    device_orientation::orient_permission(p.answer, world_platform().orient);
                }
            }
            Ok(Some(0))
        }
        Command::SENSOR_LOCATE => {
            geolocation::start_watching(world_platform().geolocation);
            ask_turn(s)
        }
        // Events the kernel posts, not commands it takes.
        Command::SENSOR_START | Command::SENSOR_STOP => Ok(None),
        _ => Ok(None),
    }
}
