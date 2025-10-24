#!/usr/bin/env pwsh
#Requires -Version 5.0

# Builds the One ROM Studio application and NSIS installers for Windows.
#
# Pre-requisites:
# - Rust:
#
# ```powershell
#   Invoke-WebRequest -Uri https://win.rustup.rs/ -OutFile rustup-init.exe
#   .\rustup-init.exe -y
# ```

$ErrorActionPreference = "Stop"

#
# Setup
#

# Install the Rust targets
rustup target add x86_64-pc-windows-msvc

# Install cargo-packager if not already installed
cargo install cargo-packager --locked

#
# Clean previous builds
#

cargo clean --target x86_64-pc-windows-msvc
Remove-Item -Path "dist\*.exe" -Force -ErrorAction SilentlyContinue

#
# Intel silicon (x86_64)
#

# Build One ROM Studio
$env:PACKAGER_TARGET = "x86_64-pc-windows-msvc"
Write-Host "Building for target: $env:PACKAGER_TARGET"
cargo build --release --target $env:PACKAGER_TARGET | Out-Host

# Package as NSIS installer
Write-Host "Packaging NSIS installer for target: $env:PACKAGER_TARGET"
cargo packager --release --target $env:PACKAGER_TARGET --formats nsis | Out-Host

Write-Host "Windows x86_64 build complete."
