# @pxl/native/modules/platform/ios/PxlPlatform.podspec
#
# The device's half of the client's services (ts/client/src/service/, the
# .native files): what the shell asks of the system — storage, the
# clipboard, text, the compass — and the kernel's own calls besides xpute's
# (`import PxlNative`, the transport's decoder). The core is built and linked by xpute's pod (@xpute/native,
# Xpute.podspec), which puts the kernel's include/ beside xpute's headers.

require 'json'

Pod::Spec.new do |s|
  s.name = 'PxlPlatform'
  s.version = '0.0.0'
  s.summary = 'The platform the pixelet shell asks of on a device.'
  # The site's (spec/spec/route.json `origins`).
  s.homepage = JSON.parse(File.read(File.join(__dir__, '../../../../../spec/spec/route.json')))['origins']['SITE']['url']
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

  s.pod_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => '"$(PODS_CONFIGURATION_BUILD_DIR)/Xpute/include"',
  }
end
