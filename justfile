name := 'yall-cosmic'
appid := 'com.github.pop-os.yall-cosmic-applet'

rootdir := ''
prefix := '/usr'

base-dir := absolute_path(clean(rootdir / prefix))

bin-src := 'target' / 'release' / name
bin-dst := base-dir / 'bin' / name

desktop := name + '.desktop'
desktop-src := 'resources' / desktop
desktop-dst := clean(rootdir / prefix) / 'share' / 'applications' / desktop

appdata := 'app.metainfo.xml'
appdata-src := 'resources' / appdata
appdata-dst := clean(rootdir / prefix) / 'share' / 'appdata' / appdata

icons-src := 'resources' / 'icons' / 'hicolor'
icons-dst := clean(rootdir / prefix) / 'share' / 'icons' / 'hicolor'

# Icon files for different sizes
icon-16-src := icons-src / '16x16' / 'apps' / 'icon.png'
icon-16-dst := icons-dst / '16x16' / 'apps' / appid + '.png'

icon-32-src := icons-src / '32x32' / 'apps' / 'icon.png'
icon-32-dst := icons-dst / '32x32' / 'apps' / appid + '.png'

icon-48-src := icons-src / '48x48' / 'apps' / 'icon.png'
icon-48-dst := icons-dst / '48x48' / 'apps' / appid + '.png'

icon-64-src := icons-src / '64x64' / 'apps' / 'icon.png'
icon-64-dst := icons-dst / '64x64' / 'apps' / appid + '.png'

icon-128-src := icons-src / '128x128' / 'apps' / 'icon.png'
icon-128-dst := icons-dst / '128x128' / 'apps' / appid + '.png'

icon-svg-src := icons-src / 'scalable' / 'apps' / 'icon.svg'
icon-svg-dst := icons-dst / 'scalable' / 'apps' / appid + '.svg'

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

# Run the application for testing purposes
run *args:
    env RUST_BACKTRACE=full cargo run --release {{args}}

# Installs files
install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 {{desktop-src}} {{desktop-dst}}
    install -Dm0644 {{appdata-src}} {{appdata-dst}}
    install -Dm0644 {{icon-16-src}} {{icon-16-dst}}
    install -Dm0644 {{icon-32-src}} {{icon-32-dst}}
    install -Dm0644 {{icon-48-src}} {{icon-48-dst}}
    install -Dm0644 {{icon-64-src}} {{icon-64-dst}}
    install -Dm0644 {{icon-128-src}} {{icon-128-dst}}
    install -Dm0644 {{icon-svg-src}} {{icon-svg-dst}}
    @echo "Installation complete. You may need to run 'gtk-update-icon-cache' or restart your desktop session to see the icon."

# Uninstalls installed files
uninstall:
    -rm -f {{bin-dst}}
    -rm -f {{desktop-dst}}
    -rm -f {{appdata-dst}}
    -rm -f {{icon-16-dst}}
    -rm -f {{icon-32-dst}}
    -rm -f {{icon-48-dst}}
    -rm -f {{icon-64-dst}}
    -rm -f {{icon-128-dst}}
    -rm -f {{icon-svg-dst}}
    @echo "Uninstall complete. You may want to run 'gtk-update-icon-cache' to update the icon cache."

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

