set dotenv-load
set dotenv-filename := ".distro_config.env"

name := 'cosmic-initial-setup'
export APPID := 'com.system76.CosmicInitialSetup'
export DISABLE_IF_EXISTS := env('DISABLE_IF_EXISTS', '/cdrom/casper/filesystem.squashfs')

prefix := '/usr'

# Settings that can be overridden/managed by configure.py
build-jobs := if env('DISTRO_JOBS') == '0' { 'default' } else { env('DISTRO_JOBS') }
install-dir := env('DISTRO_INSTALLDIR', '')
bin-dir := env('DISTRO_BINDIR', prefix / 'bin')
data-dir := env('DISTRO_DATADIR', prefix / 'share')
sysconf-dir := env('DISTRO_SYSCONFDIR', 'etc')
xdg-dir := env('DISTRO_XDGDIR', sysconf-dir / 'xdg')
polkit-dir := env('DISTRO_POLKITDIR', data-dir / 'polkit-1')

cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name
bin-dst := install-dir / bin-dir / name
icons-dir := data-dir / 'icons' / 'hicolor' / 'scalable' / 'apps'

polkit-rules-src := 'res' / '20-cosmic-initial-setup.rules'
polkit-rules-dst := install-dir / polkit-dir / 'rules.d' / '20-cosmic-initial-setup.rules'

desktop-entry := APPID + '.desktop'
desktop-src := 'res' / desktop-entry
desktop-dst := install-dir / data-dir / 'applications' / desktop-entry

icon-src := 'res' / 'icon.svg'
icon-dst := install-dir / icons-dir / APPID + '.svg'

autostart-entry := APPID + '.Autostart.desktop'
autostart-src := 'res' / autostart-entry
autostart-dst := install-dir / xdg-dir / 'autostart' / desktop-entry

layouts-src := 'res' / 'layouts'
layouts-dir := data-dir / 'cosmic-layouts'
layouts-dst := install-dir / layouts-dir

export COSMIC_LAYOUTS_DIR := layouts-dir

themes-src := 'res' / 'themes'
themes-dst := install-dir / data-dir / 'cosmic' / 'cosmic-themes'

export CARGO_BUILD_JOBS := build-jobs

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Profile memory usage with heaptrack
heaptrack:
    cargo heaptrack --profile release-with-debug

dev *args:
    cargo fmt
    just run {{args}}

# Run with debug logs
run *args:
    env RUST_LOG=cosmic_initial_setup=info RUST_BACKTRACE=full cargo run --release {{args}}

# Installs files
install: install-themes install-layouts
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 {{icon-src}} {{icon-dst}}
    install -Dm0644 {{desktop-src}} {{desktop-dst}}
    install -Dm0644 {{autostart-src}} {{autostart-dst}}
    install -Dm0644 {{polkit-rules-src}} {{polkit-rules-dst}}


install-layouts:
    rm -rf {{layouts-dst}}
    cp -rp {{layouts-src}} {{layouts-dst}}

install-themes:
    #!/bin/sh
    set -ex
    mkdir -p {{themes-dst}}
    for theme in $(find {{themes-src}} -type f); do
        install -Dm0644 ${theme} {{themes-dst}}
    done

# Uninstalls installed files
uninstall:
    rm -rf {{desktop-dst}} {{polkit-rules-dst}} {{icon-dst}} {{themes-dst}} {{layouts-dst}} {{bin-dst}}

# Vendor dependencies locally
vendor:
    #!/usr/bin/env bash
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    echo >> .cargo/config.toml
    echo '[env]' >> .cargo/config.toml
    if [ -n "${SOURCE_DATE_EPOCH}" ]
    then
        source_date="$(date -d "@${SOURCE_DATE_EPOCH}" "+%Y-%m-%d")"
        echo "VERGEN_GIT_COMMIT_DATE = \"${source_date}\"" >> .cargo/config.toml
    fi
    if [ -n "${SOURCE_GIT_HASH}" ]
    then
        echo "VERGEN_GIT_SHA = \"${SOURCE_GIT_HASH}\"" >> .cargo/config.toml
    fi
    tar pcf vendor.tar .cargo vendor
    rm -rf .cargo vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar
