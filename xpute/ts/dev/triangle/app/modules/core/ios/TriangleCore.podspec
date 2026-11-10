# xpute-triangle-app/modules/core/ios/TriangleCore.podspec
#
# Builds the app's core as the app builds (script/core-build.py). @xpute/native's
# pod links what this leaves.

Pod::Spec.new do |s|
  s.name = 'TriangleCore'
  s.version = '0.1.0'
  s.summary = "Builds the app's core."
  s.homepage = 'https://github.com/canplane/xpute'
  s.license = { type: 'MIT' }
  s.author = 'xpute'
  s.source = { git: '' }
  s.platforms = { ios: '16.4' }
  s.static_framework = true
  s.swift_version = '5.9'
  s.source_files = '*.swift'

  # cargo knows whether the core is current, so this runs on every build.
  # rustup puts its tools in the home directory, which Xcode's PATH does not name.
  app = File.expand_path('../../..', __dir__)
  s.script_phase = {
    name: 'Build the core',
    script: "export PATH=\"$HOME/.cargo/bin:$PATH\"; exec \"#{app}/script/core-build.py\" --app \"#{app}\"",
    execution_position: :before_compile,
    always_out_of_date: '1',
  }
end
