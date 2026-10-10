# @pxl/native/modules/platform/ios/PxlPlatform.podspec
#
# The device's half of the app's platform (apps/native/src/platform/): the
# view the kernel draws into, the kernel linked with it, and what the shell
# asks of the system — storage, the clipboard, text. The kernel
# is in the guest (crates/client-bracket/native), built by this pod's script phase
# (apps/native/script/build-native.ts) for the configuration and the SDK the
# app is, into a directory each has of its own, which the app links from and
# Swift imports it from (`import PxlNative`, the guest's include/). Not named
# PxlNative: a pod is a Swift module of its own name, and that one is the
# guest's.

Pod::Spec.new do |s|
  s.name = 'PxlPlatform'
  s.version = '0.0.0'
  s.summary = 'The pixelet kernel on a device, and the platform it asks of.'
  s.homepage = 'https://plei.me'
  s.license = { type: 'UNLICENSED' }
  s.author = 'pixelet'
  s.source = { git: '' }
  s.platforms = { ios: '16.4' }
  s.static_framework = true
  s.swift_version = '5.9'

  s.dependency 'ExpoModulesCore'
  s.dependency 'ExpoUI'
  s.source_files = '*.swift'

  # Run on every build: cargo is what knows whether the kernel is current,
  # and answers in a moment when it is. rustup's and Deno's installers put
  # their tools in the home directory, which Xcode's PATH does not name.
  s.script_phase = {
    name: 'Build the kernel',
    script: 'export PATH="$HOME/.cargo/bin:$HOME/.deno/bin:$PATH"; exec "$PODS_TARGET_SRCROOT/../../../script/build-native.ts"',
    execution_position: :before_compile,
    always_out_of_date: '1',
  }
  s.pod_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => '"$(PODS_TARGET_SRCROOT)/../../../../../crates/client-bracket/native/include"',
  }
  # The simulator the kernel is built for is an Apple silicon Mac's, arm64
  # alone (build-native.ts); a Release build makes every architecture it is
  # not told otherwise, and an Intel slice would link no kernel.
  s.user_target_xcconfig = {
    'LIBRARY_SEARCH_PATHS' => '$(inherited) "$(PODS_CONFIGURATION_BUILD_DIR)/PxlNative"',
    'OTHER_LDFLAGS' => '$(inherited) -lpxl_native',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'x86_64',
  }
end
