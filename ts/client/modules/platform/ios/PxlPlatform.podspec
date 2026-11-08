# @pxl/native/modules/platform/ios/PxlPlatform.podspec
#
# The device's half of the client's services on iOS (the .native files in
# ts/client/src/service/platform/). It answers what the UI asks of the system,
# such as storage, the clipboard, text and the compass, and makes the core's own
# calls besides xpute's (`import PxlNative`, the transport's decoder). xpute's
# pod (@xpute/native, Xpute.podspec) builds and links the core, and puts the
# core's include/ beside xpute's headers.

require 'json'

Pod::Spec.new do |s|
  s.name = 'PxlPlatform'
  s.version = '0.0.0'
  s.summary = 'The platform the pixelet UI asks of on a device.'
  # The site's URL (spec/spec/route.json `origins`).
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
