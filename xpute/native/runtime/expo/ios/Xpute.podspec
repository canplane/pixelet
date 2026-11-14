# @xpute/native-runtime/ios/Xpute.podspec
#
# This is xpute's module on an iPhone (*.swift), and the app's native crate
# linked with it: the guest with the runtime the app composes around it, as
# the app's package.json `xpute.guest.runtime` names it. The app builds it
# into this pod's directory, for the configuration and the SDK the app is
# built as, and the app links from there. Swift imports xpute's C ABI
# (`import CXpute`).

require 'json'

# The app is the directory over its ios/, where the pods are installed.
app = File.expand_path('..', Pod::Config.instance.installation_root.to_s)
guest = JSON.parse(File.read(File.join(app, 'package.json'))).dig('xpute', 'guest')
raise "#{app}/package.json names no xpute.guest" unless guest
# This is cargo's name for the crate's library.
lib = guest['runtime'].tr('-', '_')
include = '"$(PODS_CONFIGURATION_BUILD_DIR)/Xpute/include"'

Pod::Spec.new do |s|
  s.name = 'Xpute'
  s.version = '0.1.0'
  s.summary = "xpute's native module, and the app's native crate linked with it."
  s.homepage = 'https://github.com/canplane/xpute'
  s.license = { type: 'MIT' }
  s.author = 'xpute'
  s.source = { git: '' }
  s.platforms = { ios: '16.4' }
  s.static_framework = true
  s.swift_version = '5.9'

  s.dependency 'ExpoModulesCore'
  s.source_files = '*.swift'

  # A file is written only where it differs, as a newer header rebuilds every
  # Swift module that imports it.
  s.script_phase = {
    name: 'Put the C ABI where Swift imports it',
    script: <<~'SH',
      dir="$PODS_CONFIGURATION_BUILD_DIR/Xpute/include/CXpute"
      mkdir -p "$dir"
      for src in "$PODS_TARGET_SRCROOT/CXpute/xpute.h" "$PODS_TARGET_SRCROOT/CXpute/module.modulemap"; do
        cmp -s "$src" "$dir/$(basename "$src")" || cp "$src" "$dir/"
      done
    SH
    execution_position: :before_compile,
    always_out_of_date: '1',
  }
  s.pod_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => include,
  }
  # The simulator the guest is built for is an Apple silicon Mac's, arm64
  # alone; a Release build makes every architecture it is not
  # told otherwise, and an Intel slice would link no guest.
  s.user_target_xcconfig = {
    # Xcode's scan of the app's modules resolves CXpute from here too.
    'SWIFT_INCLUDE_PATHS' => "$(inherited) #{include}",
    'LIBRARY_SEARCH_PATHS' => '$(inherited) "$(PODS_CONFIGURATION_BUILD_DIR)/Xpute"',
    'OTHER_LDFLAGS' => "$(inherited) -l#{lib}",
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'x86_64',
  }
end
