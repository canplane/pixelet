// @pxl/native/modules/pxl-surface/ios/PxlSensorModule.swift

import CoreLocation
import ExpoModulesCore

/// What the device's sensors can do, asked once at boot
/// (apps/native/src/platform/sensor.ts): whether it has a compass at all,
/// which expo-location only says by failing a watch — and a failed watch
/// reads as a compass lost, not one never there.
public class PxlSensorModule: Module {
  public func definition() -> ModuleDefinition {
    Name("PxlSensor")

    Function("headingAvailable") { () -> Bool in
      CLLocationManager.headingAvailable()
    }
  }
}
