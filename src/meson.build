cargo_env = environment()

# App specific env vars
cargo_env.append('APP_ID', application_id)
cargo_env.append('DEVLOPMENT_BUILD', development_build.to_string())
cargo_env.append('VERSION', version + version_suffix)
cargo_env.append('GETTEXT_PACKAGE', gettext_package)
cargo_env.append('LOCALEDIR', localedir)
cargo_env.append('LIBEXECDIR', libexecdir)
cargo_env.append('RESOURCES_FILE', pkgdatadir / 'resources.gresource')

# Cargo env vars
cargo_env.append('CARGO_HOME', meson.project_build_root() / 'cargo')

rust_target = profile
if get_option('profile') == 'dev'
  rust_target = 'debug'
endif

cargo_options = [
  '--manifest-path', meson.project_source_root() / 'Cargo.toml',
  '--target-dir', meson.project_build_root() / 'src',
  '--profile', profile
]

test(
  'Cargo tests (main application)',
  cargo,
  args: ['test', cargo_options],
  timeout: 3600,
  env: cargo_env,
)

cargo_build = custom_target(
  'cargo-build',
  depends: resources,
  build_by_default: true,
  build_always_stale: true,
  output: rust_target,
  console: true,
  env: cargo_env,
  command: [
    cargo,
    'build',
    cargo_options,
  ],
)

copy_binary = custom_target(
  'cp-binary',
  depends: cargo_build,
  build_by_default: true,
  build_always_stale: true,
  install: true,
  install_dir: bindir,
  output: meson.project_name(),
  command: [
    'cp',
    'src' / rust_target / meson.project_name(),
    '@OUTPUT@',
  ],
)

copy_kill_binary = custom_target(
  'cp-kill-binary',
  depends: cargo_build,
  build_by_default: true,
  build_always_stale: true,
  install: true,
  install_dir: libexecdir,
  output: meson.project_name() + '-kill',
  command: [
    'cp',
    'src' / rust_target / meson.project_name() + '-kill',
    '@OUTPUT@',
  ],
)

copy_processes_binary = custom_target(
  'cp-processes-binary',
  depends: cargo_build,
  build_by_default: true,
  build_always_stale: true,
  install: true,
  install_dir: libexecdir,
  output: meson.project_name() + '-processes',
  command: [
    'cp',
    'src' / rust_target / meson.project_name() + '-processes',
    '@OUTPUT@',
  ],
)

copy_adjust_binary = custom_target(
  'cp-adjust-binary',
  depends: cargo_build,
  build_by_default: true,
  build_always_stale: true,
  install: true,
  install_dir: libexecdir,
  output: meson.project_name() + '-adjust',
  command: [
    'cp',
    'src' / rust_target / meson.project_name() + '-adjust',
    '@OUTPUT@',
  ],
)
