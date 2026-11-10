# @xpute/gpu/ios/XputeGpu.podspec
#
# The GPU's module on an iPhone: the view a core's surface draws into. The
# symbols it calls are in the core that xpute's pod links. Its header is put
# beside xpute's, as a module of its own (`import CXputeGpu`).

Pod::Spec.new do |s|
  s.name = 'XputeGpu'
  s.version = '0.1.0'
  s.summary = "The GPU's module on an iPhone: the view a core's surface draws into."
  s.homepage = 'https://github.com/canplane/xpute'
  s.license = { type: 'MIT' }
  s.author = 'xpute'
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
      dir="$PODS_CONFIGURATION_BUILD_DIR/Xpute/include/CXputeGpu"
      mkdir -p "$dir"
      for src in "$PODS_TARGET_SRCROOT/CXputeGpu/xpute_gpu.h" "$PODS_TARGET_SRCROOT/CXputeGpu/module.modulemap"; do
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
