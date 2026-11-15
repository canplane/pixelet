# @pxl/gpu/expo/ios/PxlGpu.podspec
#
# The GPU's module on an iPhone: the view the guest's surface draws into. The
# symbols it calls are in the library xpute's pod links. Its header is put
# beside xpute's, as a module of its own (`import CPxlGpu`).

Pod::Spec.new do |s|
  s.name = 'PxlGpu'
  s.version = '0.0.0'
  s.summary = "The GPU's module on an iPhone: the view the guest's surface draws into."
  # The site's URL (spec/spec/route.json5 `origins`), as the client's route.spec.ts writes it.
  s.homepage = File.read(File.join(__dir__, '../../../client/src/service/origin/route.spec.ts'))[/SITE_URL = "([^"]+)"/, 1]
  s.license = { type: 'UNLICENSED' }
  s.author = 'pixelet'
  s.source = { git: '' }
  s.platforms = { ios: '16.4' }
  s.static_framework = true
  s.swift_version = '5.9'

  s.dependency 'ExpoModulesCore'
  s.dependency 'Xpute'
  s.source_files = '*.swift'

  s.script_phase = {
    name: 'Put the C ABI beside xpute\'s',
    script: <<~'SH',
      dir="$PODS_CONFIGURATION_BUILD_DIR/Xpute/include/CPxlGpu"
      mkdir -p "$dir"
      for src in "$PODS_TARGET_SRCROOT/CPxlGpu/pxl_gpu.h" "$PODS_TARGET_SRCROOT/CPxlGpu/module.modulemap"; do
        cmp -s "$src" "$dir/$(basename "$src")" || cp "$src" "$dir/"
      done
    SH
    execution_position: :before_compile,
    always_out_of_date: '1',
  }
  s.pod_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => '"$(PODS_CONFIGURATION_BUILD_DIR)/Xpute/include"',
  }
end
