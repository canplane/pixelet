# @pxl/native/modules/platform/ios/PxlPlatform.podspec
#
# The device's half of the app's platform (apps/native/src/platform/): the
# view the kernel draws into, the kernel linked with it, and what the shell
# asks of the system — storage, text, the compass. PxlNative.xcframework is
# the Rust core (crates/pxl/web/native), built by
# apps/native/script/build-native.ts before the app is. Not named PxlNative:
# a pod is a Swift module of its own name, and that one is the core's.

Pod::Spec.new do |s|
  s.name = 'PxlPlatform'
  s.version = '0.0.0'
  s.summary = 'The pixelet kernel on a device, and the platform it asks of.'
  s.homepage = 'https://plei.me'
  s.license = { type: 'UNLICENSED' }
  s.author = 'pixelet'
  s.source = { git: '' }
  s.platforms = { ios: '15.1' }
  s.static_framework = true
  s.swift_version = '5.9'

  s.dependency 'ExpoModulesCore'
  s.source_files = '*.swift'
  s.vendored_frameworks = 'PxlNative.xcframework'
end
