# onerom-studio

A GUI front-end for interacting with One ROM and managing firmware images.

It is **much** easier to let GitHub Actions build the installers for you, but if you want to build locally, follow these instructions.

There are steps missing below which are done by CI, such as generating a custom .dmg icon on Mac.

## Dependencies

Assuming building Windows target on a Debian-based Linux distribution.

For building linux targets:

```bash
sudo apt install libudev-dev
```

Install the Rust Windows targets:

```bash
rustup target add x86_64-pc-windows-gnu
```

Install ming-w64 for Windows builds:

```bash
sudo apt install mingw-w64
```

To run th ARM64 Linux build you really need to be building on the platform (or using CI), as libudev is a pain to build as part of the cross compilation.

## Packaging

### Dependencies

Assumes all commands are run from this project directory.

Install cargo packager:

```bash
cargo install cargo-packager --locked
```

Install NSIS for Windows installer creation:

```bash
sudo apt install nsis
```

Install Windows GNU toolchain:

```bash
sudo apt install mingw-w64
```

### Windows (x86_64)

These instructions are for building Windows installers on a Linux machine.  It produces an NSIS installer, which is more modern than the older MSI format.

```bash
PACKAGER_TARGET=x86_64-pc-windows-gnu cargo build --release --target $PACKAGER_TARGET
PACKAGER_TARGET=x86_64-pc-windows-gnu cargo packager --release --target $PACKAGER_TARGET --formats nsis
```

```bash
$ ls -l dist/*.exe
-rw-r--r-- 1 pdf pdf 10081302 Oct 21 09:05 dist/onerom-studio_0.1.0_x64-setup.exe
```

Note that this mechanism builds an app using GNU, which leads to a 50% larger installer and 5x larger binary than the MSVC version.  However, the MVSC target is much harder to build on linux.  For testing, using Linux is fine, but for releases, use a Windows machine with the MSVC target (or the CI).

## Linux (x86_64)

```bash
env PACKAGER_TARGET=x86_64-unknown-linux-gnu cargo build --release --target $PACKAGER_TARGET
env PACKAGER_TARGET=x86_64-unknown-linux-gnu cargo packager --release --target $PACKAGER_TARGET --formats deb
```

```bash
$ ls -l dist/*.deb
-rw-r--r-- 1 pdf pdf 13741434 Oct 21 09:06 dist/onerom-studio_0.1.0_amd64.deb
```

## Linux (ARM64)

```bash
env PACKAGER_TARGET=aarch64-unknown-linux-gnu cargo build --release --target $PACKAGER_TARGET
env PACKAGER_TARGET=aarch64-unknown-linux-gnu cargo packager --release --target $PACKAGER_TARGET --formats deb
```

## Mac - Intel

```bash
env PACKAGER_TARGET=x86_64-apple-darwin LIBUSB_STATIC=1 cargo build --release --target $PACKAGER_TARGET
env PACKAGER_TARGET=x86_64-apple-darwin cargo packager --release --target $PACKAGER_TARGET --formats dmg
```

## Mac - Apple Silicon

```bash
env PACKAGER_TARGET=aarch64-apple-darwin LIBUSB_STATIC=1 cargo build --release --target $PACKAGER_TARGET
env PACKAGER_TARGET=aarch64-apple-darwin cargo packager --release --target $PACKAGER_TARGET --formats dmg
```