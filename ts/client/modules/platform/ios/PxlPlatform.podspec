# @pxl/client/modules/platform/ios/PxlPlatform.podspec
#
# The device's half of the client's services on iOS (the .native files in
# ts/client/src/service/platform/). It answers what the UI asks of the system,
# such as storage, the clipboard, text and the compass, and makes the native
# library's own calls besides xpute's (`import PxlNative`, the transport's
# decoder). It builds that library, the core with its runtime
# (script/core-build.py), which xpute's pod links, and puts the library's
# include/ beside xpute's headers.

require 'json'

Pod::Spec.new do |s|
  s.name = 'PxlPlatform'
  s.version = '0.0.0'
  s.summary = 'The platform the pixelet UI asks of on a device.'
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
  s.dependency 'PxlGpu'
  s.source_files = '*.swift'

  # This runs on every build: cargo is what knows whether the core is current,
  # and answers in a moment when it is. rustup's installer puts its tools in the
  # home directory, which Xcode's PATH does not name.
  app = File.expand_path('../../..', __dir__)
  s.script_phase = {
    name: 'Build the core',
    script: "export PATH=\"$HOME/.cargo/bin:$PATH\"; exec \"#{app}/script/core-build.py\" --app \"#{app}\"",
    execution_position: :before_compile,
    always_out_of_date: '1',
  }

  s.pod_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => '"$(PODS_CONFIGURATION_BUILD_DIR)/Xpute/include"',
  }
end
