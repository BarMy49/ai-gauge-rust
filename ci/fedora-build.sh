#!/bin/bash
# Build, test and package AI Gauge as an .rpm inside a Fedora container.
# Usage (from the repository root):
#   docker run --rm --platform linux/arm64 -v "$PWD:/src" -w /src fedora:43 bash ci/fedora-build.sh
set -euo pipefail
dnf -y -q install gcc gcc-c++ make pkgconf-pkg-config fontconfig-devel freetype-devel \
  libxkbcommon-devel dbus-devel tar rpm-build desktop-file-utils xorg-x11-server-Xvfb xorg-x11-xauth dbus-daemon \
  dbus-tools libxkbcommon-x11 libX11 libXcursor libXi libXrandr dejavu-sans-fonts
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
. "$HOME/.cargo/env"
export CARGO_TARGET_DIR=/tmp/target
cargo test --locked
cargo build --locked --release
dbus-run-session -- xvfb-run -a "$CARGO_TARGET_DIR/release/ai-gauge" --smoke-test
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
desktop-file-validate assets/ai-gauge.desktop
mkdir -p dist
rpmbuild -bb ci/ai-gauge.spec \
  --define "_topdir /tmp/rpmbuild" \
  --define "app_version $version" \
  --define "app_bin $CARGO_TARGET_DIR/release/ai-gauge" \
  --define "app_src $PWD"
cp /tmp/rpmbuild/RPMS/*/ai-gauge-*.rpm dist/
ls -l dist/*.rpm
