#!/usr/bin/env sh
# Android Optimizer for Linux and macOS. Options pass through, e.g.: ./start.sh check
set -e
dir=$(cd "$(dirname "$0")" && pwd)
exe="$dir/android-optimizer"
[ -x "$exe" ] || exe="$dir/target/release/android-optimizer"
if [ ! -x "$exe" ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "The program is not here yet. Download the ZIP for your system from"
    echo "  https://github.com/Kushisusumita/android-optimizer/releases"
    echo "unpack it and run ./start.sh again - or install Rust from https://rustup.rs"
    exit 1
  fi
  echo "Building the program once, about a minute..."
  cargo build --release --manifest-path "$dir/Cargo.toml"
fi
cd "$dir"
exec "$exe" "$@"
