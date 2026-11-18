# @xpute/expo/ios/Xpute.podspec
#
# This is xpute's module on an iPhone (*.swift), and the app's native crate
# built and linked with it: the guest, Rust or C++, with what the runtime
# starts it with, as the app's package.json `xpute.guest` names it. Each build
# of the app builds the crate into this pod's directory, for the configuration
# and the SDK the app is built as, and the app links from there. Swift imports
# xpute's C ABI (`import CXpute`).

require 'json'
require 'shellwords'

# The app is the directory over its ios/, where the pods are installed.
app = File.expand_path('..', Pod::Config.instance.installation_root.to_s)
guest = JSON.parse(File.read(File.join(app, 'package.json'))).dig('xpute', 'guest')
raise "#{app}/package.json names no xpute.guest" unless guest
crate = guest.fetch('runtime')
# This is cargo's name for the crate's library.
lib = crate.tr('-', '_')
workspace = File.expand_path(guest.fetch('workspace'), app)
# A variant's cargo profile, then its further arguments of cargo's, as a shell
# reads them.
cargo_args = lambda do |variant, profile|
  [guest.dig('profile', variant) || profile, *(guest.dig('args', variant) || [])].shelljoin
end
include = '"$(PODS_CONFIGURATION_BUILD_DIR)/Xpute/include"'

Pod::Spec.new do |s|
  s.name = 'Xpute'
  s.version = '0.1.0'
  s.summary = "xpute's native module, and the app's native crate built and linked with it."
  s.homepage = 'https://github.com/canplane/xpute'
  s.license = { type: 'MIT', file: '../LICENSE' }
  s.author = 'xpute'
  s.source = { git: '' }
  s.platforms = { ios: '16.4' }
  s.static_framework = true
  s.swift_version = '5.9'

  s.dependency 'ExpoModulesCore'
  s.source_files = '*.swift'

  s.script_phases = [
    # A file is written only where it differs, as a newer header rebuilds
    # every Swift module that imports it.
    {
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
    },
    # This runs on every build: cargo is what knows whether the crate is
    # current, and answers in a moment when it is. The library is copied only
    # when cargo wrote a new one, as the app links again whenever the library
    # it takes is newer than the one it has.
    {
      name: 'Build the guest',
      script: <<~SH,
        set -e
        # rustup's installer puts its tools in the home directory, which
        # Xcode's PATH does not name.
        PATH="$HOME/.cargo/bin:$PATH"
        # Xcode's SDKROOT names the iPhone's SDK, and a build script cargo
        # compiles for this Mac would link against it; rustc finds each
        # target's SDK itself.
        unset SDKROOT
        # The device, and the simulator on an Apple silicon Mac.
        case "$PLATFORM_NAME" in
          iphoneos) target=aarch64-apple-ios ;;
          iphonesimulator) target=aarch64-apple-ios-sim ;;
          *) echo "error: xpute has no Rust target for the SDK $PLATFORM_NAME" >&2; exit 1 ;;
        esac
        if [ "$CONFIGURATION" = Release ]; then
          set -- #{cargo_args.call('release', 'release')}
        else
          set -- #{cargo_args.call('debug', 'dev')}
        fi
        profile=$1
        shift
        cd #{workspace.shellescape}
        # A crate may declare several crate types. This asks cargo for the
        # static library alone, by name.
        cargo rustc --crate-type staticlib --profile "$profile" --package #{crate.shellescape} --target "$target" "$@"
        # Cargo's own rule says where the library lies: CARGO_TARGET_DIR, else
        # the workspace's target/, under the profile's directory, which cargo
        # names `debug` for dev.
        if [ "$profile" = dev ]; then profile=debug; fi
        src="${CARGO_TARGET_DIR:-$PWD/target}/$target/$profile/lib#{lib}.a"
        dst="$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib#{lib}.a"
        if [ ! -e "$dst" ] || [ "$src" -nt "$dst" ]; then
          mkdir -p "${dst%/*}"
          cp "$src" "$dst"
        fi
      SH
      execution_position: :before_compile,
      always_out_of_date: '1',
    },
  ]
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
