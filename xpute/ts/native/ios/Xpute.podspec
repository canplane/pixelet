# @xpute/native/ios/Xpute.podspec
#
# This is xpute's module on an iPhone (*.swift), and the core linked with it.
# The core is the app's core crate, as the app's package.json `xpute.core` names
# it, and script/core-build.py builds it for the configuration and the SDK the
# app is built as, into a directory each has of its own, which the app links
# from. Swift imports xpute's C ABI from there (`import CXpute`), and the
# crate's own headers beside it, where the crate has an include/.

require 'json'

# The app is the directory over its ios/, where the pods are installed.
app = File.expand_path('..', Pod::Config.instance.installation_root.to_s)
core = JSON.parse(File.read(File.join(app, 'package.json'))).dig('xpute', 'core')
raise "#{app}/package.json names no xpute.core" unless core
# This is cargo's name for the crate's library.
lib = core['package'].tr('-', '_')
include = '"$(PODS_CONFIGURATION_BUILD_DIR)/Xpute/include"'

Pod::Spec.new do |s|
  s.name = 'Xpute'
  s.version = '0.0.0'
  s.summary = "xpute's native module, and the core linked with it."
  s.homepage = 'https://github.com/canplane/xpute'
  s.license = { type: 'UNLICENSED' }
  s.author = 'xpute'
  s.source = { git: '' }
  s.platforms = { ios: '16.4' }
  s.static_framework = true
  s.swift_version = '5.9'

  s.dependency 'ExpoModulesCore'
  s.source_files = '*.swift'

  # This runs on every build: cargo is what knows whether the core is current,
  # and answers in a moment when it is. rustup's installer puts its tools in the
  # home directory, which Xcode's PATH does not name.
  s.script_phase = {
    name: 'Build the core',
    script: "export PATH=\"$HOME/.cargo/bin:$PATH\"; exec \"$PODS_TARGET_SRCROOT/../script/core-build.py\" --app \"#{app}\"",
    execution_position: :before_compile,
    always_out_of_date: '1',
  }
  s.pod_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => include,
  }
  # The simulator the core is built for is an Apple silicon Mac's, arm64
  # alone (core-build.py); a Release build makes every architecture it is not
  # told otherwise, and an Intel slice would link no core.
  s.user_target_xcconfig = {
    # Xcode's scan of the app's modules resolves CXpute from here too.
    'SWIFT_INCLUDE_PATHS' => "$(inherited) #{include}",
    'LIBRARY_SEARCH_PATHS' => '$(inherited) "$(PODS_CONFIGURATION_BUILD_DIR)/Xpute"',
    'OTHER_LDFLAGS' => "$(inherited) -l#{lib}",
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'x86_64',
  }
end
