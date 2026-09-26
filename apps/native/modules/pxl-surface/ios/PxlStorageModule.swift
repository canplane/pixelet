// @pxl/native/modules/pxl-surface/ios/PxlStorageModule.swift

import ExpoModulesCore

/// What the app keeps across launches (apps/native/src/platform/storage.ts):
/// UserDefaults, the system's store for an app's small values, as a page's
/// localStorage is the browser's.
public class PxlStorageModule: Module {
  public func definition() -> ModuleDefinition {
    Name("PxlStorage")

    Function("get") { (key: String) -> String? in
      UserDefaults.standard.string(forKey: key)
    }

    Function("set") { (key: String, value: String) in
      UserDefaults.standard.set(value, forKey: key)
    }
  }
}
