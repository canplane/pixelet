# @pxl/native/modules/pxl-surface/ios/PxlSurface.podspec
#
# The view the kernel draws into on an iPhone, and the kernel linked with it:
# PxlNative.xcframework is the Rust core (crates/pxl/web/native), built by
# apps/native/script/build-native.ts before the app is.

Pod::Spec.new do |s|
  s.name = 'PxlSurface'
  s.version = '0.0.0'
  s.summary = 'The surface the pixelet kernel draws into.'
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
