#!/bin/bash
# Build, test and package AI Gauge inside a Fedora container.
# Usage (from the repository root):
#   docker run --rm --platform linux/arm64 -v "$PWD:/src" -w /src fedora:43 bash ci/fedora-build.sh
set -euo pipefail
dnf -y -q install gcc gcc-c++ make pkgconf-pkg-config fontconfig-devel freetype-devel \
  libxkbcommon-devel dbus-devel tar xorg-x11-server-Xvfb xorg-x11-xauth dbus-daemon \
  dbus-tools libxkbcommon-x11 libX11 libXcursor libXi libXrandr dejavu-sans-fonts
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR=/tmp/target
cargo test --locked
cargo build --locked --release
dbus-run-session -- xvfb-run -a "$CARGO_TARGET_DIR/release/ai-gauge" --smoke-test
arch=$(uname -m)
name="AI-Gauge-Fedora-$arch"
rm -rf "/tmp/$name" && mkdir -p "/tmp/$name" dist
cp "$CARGO_TARGET_DIR/release/ai-gauge" assets/ai-gauge.desktop assets/ai-gauge.svg LICENSE "/tmp/$name/"
tar -czf "dist/$name.tar.gz" -C /tmp "$name"
echo "Built dist/$name.tar.gz"
