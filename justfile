set unstable

# The cargo module contains cargo recipes shared by all cosmic projects.
mod? cargo 'cargo.just'

name := 'cosmic-initial-setup'
appid := 'com.system76.CosmicInitialSetup'
rootdir := ''
prefix := '/usr'

# An optional file that when set will prevent initial-setup from loading
disable-if-exists := ''
export DISABLE_IF_EXISTS := disable-if-exists || env('DISABLE_IF_EXISTS', '')

# Installation paths
base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
desktop-entry := appid + '.desktop'
autostart-dst := rootdir / 'etc' / 'xdg' / 'autostart' / desktop-entry
autostart-entry := appid + '.Autostart.desktop'
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / desktop-entry
icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'
layouts-dst := base-dir / 'share' / 'cosmic-layouts'
polkit-rules-dst := base-dir / 'share' / 'polkit-1' / 'rules.d' / '20-cosmic-initial-setup.rules'
themes-dst := base-dir / 'share' / 'cosmic-themes'

# Default recipe which runs `just cargo build-release`
default: fetch
    @just cargo build-release

build-release: fetch
    @just cargo build-release

# Build release binary from vendored sources
build-vendored:
    @just cargo build-vendored

# Clean
clean: fetch
    @just cargo clean
    rm -rf {{layouts-dst}} {{themes-dst}} {{bin-dst}} {{icon-dst}} {{desktop-dst}} {{autostart-dst}} {{polkit-rules-dst}}

# Fetch cargo.just module if it is missing on the disk.
@fetch:
    {{ if path_exists('cargo.just') == 'false' { 'curl --proto =https --tlsv1.2 -sSf -o cargo.just https://raw.githubusercontent.com/pop-os/cosmic-justfiles/master/cargo.just' } else { 'true' } }}

# Installs files
install:
    mkdir -p {{layouts-dst}} {{themes-dst}}
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} '{{bin-dst}}'
    install -Dm0644 res/icon.svg '{{icon-dst}}'
    install -Dm0644 {{ 'res' / desktop-entry }} '{{desktop-dst}}'
    install -Dm0644 {{ 'res' / autostart-entry }} '{{autostart-dst}}'
    install -Dm0644 res/20-cosmic-initial-setup.rules '{{polkit-rules-dst}}'
    cd res/layouts; find . -type f -exec install -Dm0644 '{}' '{{layouts-dst}}/{}' \;
    find res/themes -type f -exec install -Dm0644 '{}' '{{themes-dst}}' \;

# Bump cargo version, create git commit, and create tag
tag version:
    sed -i '0,/^version/s/^version.*/version = "{{version}}"/' Cargo.toml
    cargo check
    cargo clean
    dch -D noble -v {{version}}
    git add Cargo.toml Cargo.lock debian/changelog
    git commit -m 'release: {{version}}'
    git tag -a {{version}} -m ''

# Uninstalls installed files
uninstall:
    rm -rf {{desktop-dst}} {{polkit-rules-dst}} {{icon-dst}} {{themes-dst}} {{layouts-dst}} {{bin-dst}}

# Vendor sources
vendor: fetch
    just cargo vendor
