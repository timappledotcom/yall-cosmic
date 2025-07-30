# Yall Cosmic

A compact COSMIC application for posting to multiple social media platforms (Bluesky, Mastodon, Micro.Blog, and Nostr) simultaneously. Features a clean, focused interface optimized for quick social media posting.

## Features

- **Multi-platform posting**: Post to Bluesky, Mastodon, Micro.Blog, and Nostr simultaneously
- **Secure credential storage**: All sensitive credentials are encrypted using AES-256-GCM
- **Account management**: Configure credentials for each platform with validation
- **Smart character limits**: 500 character limit with platform-specific handling (Bluesky auto-truncates at 300)
- **Native COSMIC application**: Built with libcosmic for seamless COSMIC desktop integration
- **Optimized UI**: Multi-line text editor popup with word wrapping for comfortable 500-character composition

## Setup

### Bluesky
1. Go to Settings tab
2. Enable Bluesky
3. Enter your handle (e.g., `username.bsky.social`)
4. Generate an app password at https://bsky.app/settings/app-passwords
5. Enter the app password (not your main password)

### Mastodon
1. Go to Settings tab
2. Enable Mastodon
3. Enter your instance URL (e.g., `https://mastodon.social`)
4. Generate an access token from your instance's developer settings
5. Enter the access token

### Nostr
1. Go to Settings tab
2. Enable Nostr
3. Enter your private key in hex format
4. Add relay URLs (e.g., `wss://relay.damus.io`)
5. You can add multiple relays for better reach

## Installation

### From Source
```bash
# Build the applet
just build-debug  # Use debug build for now due to renderer configuration

# Install system-wide (requires sudo for system directories)
sudo just install

# Or install to a custom location
just rootdir=/path/to/install prefix=/usr install

# Update icon cache (may be needed for the dock icon to appear)
sudo gtk-update-icon-cache /usr/share/icons/hicolor/ || true

# The applet will be installed to the system and available in COSMIC panel configuration
```

### Launching the Application
1. Install using the instructions above
2. Launch from the COSMIC applications menu or run `yall-cosmic` from terminal
3. The application will open with a compact, focused interface
4. You can minimize it to the taskbar when not in use

## Security

Yall Cosmic takes credential security seriously:

- **Encrypted Storage**: All sensitive credentials (passwords, tokens, private keys) are encrypted using AES-256-GCM before being stored
- **Key Derivation**: Encryption keys are derived using Argon2 with machine-specific entropy
- **Memory Safety**: Credentials are automatically zeroed from memory when no longer needed
- **No Plain Text**: Sensitive data is never stored in plain text on disk

The encryption key is derived from machine-specific information, making credentials tied to your specific device.

## Usage

1. Launch Yall Cosmic from the applications menu or terminal
2. Switch between Compose and Settings tabs using the buttons
3. In Compose: Type your message (max 500 characters, Bluesky posts auto-truncated at 300) and click "Post"
4. In Settings: Configure your social media accounts with input validation
5. Status messages will show posting progress and results
6. Minimize or close the window when done

## Installation

A [justfile](./justfile) is included by default for the [casey/just][just] command runner.

- `just` builds the application with the default `just build-release` recipe
- `just run` builds and runs the application
- `just install` installs the project into the system
- `just vendor` creates a vendored tarball
- `just build-vendored` compiles with vendored dependencies from that tarball
- `just check` runs clippy on the project to check for linter warnings
- `just check-json` can be used by IDEs that support LSP

## Translators

[Fluent][fluent] is used for localization of the software. Fluent's translation files are found in the [i18n directory](./i18n). New translations may copy the [English (en) localization](./i18n/en) of the project, rename `en` to the desired [ISO 639-1 language code][iso-codes], and then translations can be provided for each [message identifier][fluent-guide]. If no translation is necessary, the message may be omitted.

## Packaging

If packaging for a Linux distribution, vendor dependencies locally with the `vendor` rule, and build with the vendored sources using the `build-vendored` rule. When installing files, use the `rootdir` and `prefix` variables to change installation paths.

```sh
just vendor
just build-vendored
just rootdir=debian/yall-cosmic prefix=/usr install
```

It is recommended to build a source tarball with the vendored dependencies, which can typically be done by running `just vendor` on the host system before it enters the build environment.

## Developers

Developers should install [rustup][rustup] and configure their editor to use [rust-analyzer][rust-analyzer]. To improve compilation times, disable LTO in the release profile, install the [mold][mold] linker, and configure [sccache][sccache] for use with Rust. The [mold][mold] linker will only improve link times if LTO is disabled.

[fluent]: https://projectfluent.org/
[fluent-guide]: https://projectfluent.org/fluent/guide/hello.html
[iso-codes]: https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes
[just]: https://github.com/casey/just
[rustup]: https://rustup.rs/
[rust-analyzer]: https://rust-analyzer.github.io/
[mold]: https://github.com/rui314/mold
[sccache]: https://github.com/mozilla/sccache
