# INSTALL

one-rom requires a fairly involved toolchain to build, due to the extend of the project (embedded firmware, desktop application, webassembly, etc).  This document covers installing the toolchain and dependencies on linux (primarily focusing on an x86_64 Debian-based distribution, although notes are also provided for an ARM64 based host).  Other hosts (Mac, Windows) are possible, and it is recommended to use macOS for building One ROM Studio for Mac, and Windows for building Windows installers.

Get hold of a [supported microcontroller](README.md#supported-stm32-microcontrollers) and the [PCB](sdrr-pcb) and solder the PCB up.

Get rev E from here:

[![Order from OSH Park](https://oshpark.com/assets/badge-5b7ec47045b78aef6eb9d83b3bac6b1920de805e9a0c227658eac6e19a045b9c.png)](https://oshpark.com/shared_projects/9TJoAirm)

At this point, you can choose to install the dependencies locally, or use the [Docker container](ci/docker/README.md) to build the One ROM firmware.

0. Install pre-requisites

    ```bash
    sudo apt -y install git build-essential curl pkg-config
    ```

1. Clone the repository:

    ```bash
    git clone https://github.com/piersfinlayson/one-rom.git
    cd one-rom
    ```

2. Install the required [ARM GNU toolchain](https://developer.arm.com/downloads/-/arm-gnu-toolchain-downloads) for AArch32 bare-metal target (arm-none-eabi).

    Recommended approach - download the toolchain from ARM's developer site (this is quite large, so may take a while):

    ```bash
    wget https://developer.arm.com/-/media/Files/downloads/gnu/14.3.rel1/binrel/arm-gnu-toolchain-14.3.rel1-x86_64-arm-none-eabi.tar.xz
    tar -xvf arm-gnu-toolchain-14.3.rel1-x86_64-arm-none-eabi.tar.xz
    sudo mv arm-gnu-toolchain-14.3.rel1-x86_64-arm-none-eabi /opt/
    ```

    Or install it via your package manager, e.g., on Debian/Ubuntu:

    ```bash
    sudo apt -y install gcc-arm-none-eabi
    ```

    Or install the aarch64 version if you are on an ARM64 host.  Again update TOOLCHAIN.

    If you install using the package manager, you will need to update the `TOOLCHAIN` environment variable or variable in the [Makefile](sdrr/Makefile) to point to the correct compiler binary directory.  It should probably `/usr/bin` or similar.

2a. If on an ARM64 host you will also need x86_64-linux-gnu cross tools:

    ```bash
    sudo apt -y install gcc-x86-64-linux-gnu
    ```

3. Install curl, zip and json-x development packages (required for tests), vice (for demos), dfu-util (for STM32 DFU flashing), jq (for JSON manifest generation):

    ```bash
    sudo apt -y install dfu-util jq libcurl4-openssl-dev libzip-dev libjson-c-dev libudev-dev vice
    ```

    If you are using a different package manager, the package name may vary slightly, e.g., `libcurl-devel` on Fedora.

4. Install [Rust](https://www.rust-lang.org/tools/install):

    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
    rustup target install thumbv7em-none-eabihf
    cargo install wasm-pack
    cargo install cross
    cargo install cargo-dist
    ```

    If planning to build One ROM Studio for all possible targets (you likely only want to build a subset):
    
    ```bash
    rustup target install \
        x86_64-unknown-linux-gnu \
        aarch64-unknown-linux-gnu \
        x86_64-pc-windows-gnu \
        aarch64-pc-windows-gnullvm \
        x86_64-pc-windows-msvc \
        aarch64-pc-windows-msvc \
        x86_64-apple-darwin \
        aarch64-apple-darwin
    sudo apt -y install mingw-w64
    ```

5. Install [probe-rs](https://probe.rs/) for flashing the firmware to One ROM.

    ```bash
    curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
    probe-rs complete install
    ```

6. Connect up One ROM to your [programmer](README.md#programmer).

At this point you can follow the instructions in [Quick Start](README.md#quick-start) to build and flash the One ROM firmware.
