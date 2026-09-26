#!/usr/bin/env bash
# One-time setup after the learn-rust dev container is created.
set -euo pipefail

cd "$(dirname "$0")/../.."

# Named volumes are created root-owned; hand them to the vscode user.
sudo chown -R "$(id -u):$(id -g)" learn-rust/target target

# databroker-proto generates Rust code from the .proto files in this submodule.
git submodule update --init

# Install the pinned toolchain (learn-rust/rust-toolchain.toml) and
# pre-compile all dependencies so the first `cargo rl` is quick.
cd learn-rust
rustup show active-toolchain
cargo build -p rl -p exercises

echo
echo "Ready. Open learn-rust/README.md, then run:  cd learn-rust && cargo rl"
