# @pxl/client/modules/platform/ios/PxlPlatform.podspec
#
# The native half of the client's services on iOS (the .native files in
# ts/client/src/service/platform/). It answers what the UI asks of the system,
# such as storage, the clipboard, text and the compass, and makes the native
# library's own calls besides xpute's (`import PxlNative`, the transport's
# decoder). xpute's pod builds that library, the core with its runtime; this
# one puts the library's include/ beside xpute's headers, and numbers the
# source paths a release's library holds (script/number-paths.py).

require 'json'

app = File.expand_path('../../..', __dir__)
guest = JSON.parse(File.read(File.join(app, 'package.json'))).dig('xpute', 'guest')
workspace = File.expand_path(guest.fetch('workspace'), app)
crate = guest.fetch('runtime')
# This is cargo's name for the library xpute's pod builds.
lib = crate.tr('-', '_')
# The crate's own directory, as cargo has it. rustup's installer puts its
# tools in the home directory, which the PATH here need not name, and `env`
# finds cargo on the PATH it is given.
metadata = IO.popen(['env', "PATH=#{Dir.home}/.cargo/bin:#{ENV['PATH']}", 'cargo', 'metadata', '--no-deps', '--format-version', '1'], chdir: workspace, &:read)
raise "cargo metadata failed in #{workspace}" unless $?.success?
manifest = JSON.parse(metadata)['packages'].find { |package| package['name'] == crate }.fetch('manifest_path')

Pod::Spec.new do |s|
  s.name = 'PxlPlatform'
  s.version = '0.0.0'
  s.summary = 'The platform the pixelet UI asks of on iOS.'
  # The site's URL (spec/spec/route.json5 `origins`), as the client's route.spec.ts writes it.
  s.homepage = File.read(File.join(__dir__, '../../../src/service/origin/route.spec.ts'))[/SITE_URL = "([^"]+)"/, 1]
  s.license = { type: 'UNLICENSED' }
  s.author = 'pixelet'
  s.source = { git: '' }
  s.platforms = { ios: '16.4' }
  s.static_framework = true
  s.swift_version = '5.9'

  s.dependency 'ExpoModulesCore'
  s.dependency 'ExpoUI'
  s.dependency 'Xpute'
  s.source_files = '*.swift'

  # The headers are written only where they differ, as a newer header rebuilds
  # every Swift module that imports it. The numbering runs on every release
  # build: over paths already numbered it changes nothing, and the table of
  # the run that numbered them stands. It asks cargo for the workspace's
  # packages, and rustup's installer puts its tools in the home directory,
  # which Xcode's PATH does not name.
  s.script_phase = {
    name: 'Put the library\'s headers and number its paths',
    script: <<~SH,
      set -e
      dir="$PODS_CONFIGURATION_BUILD_DIR/Xpute/include"
      mkdir -p "$dir"
      for src in "#{File.dirname(manifest)}/include"/*; do
        cmp -s "$src" "$dir/$(basename "$src")" || cp "$src" "$dir/"
      done
      if [ "$CONFIGURATION" = Release ]; then
        PATH="$HOME/.cargo/bin:$PATH"
        "#{app}/script/number-paths.py" "$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib#{lib}.a" "#{app}/core.paths.$PLATFORM_NAME.json" "#{workspace}"
      fi
    SH
    execution_position: :before_compile,
    always_out_of_date: '1',
  }

  s.pod_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => '"$(PODS_CONFIGURATION_BUILD_DIR)/Xpute/include"',
  }
end
