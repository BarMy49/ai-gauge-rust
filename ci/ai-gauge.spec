# Packages the binary that ci/fedora-build.sh has already built and tested.
Name:           ai-gauge
Version:        %{app_version}
Release:        1%{?dist}
Summary:        Native desktop AI usage and balance monitor
License:        MIT
URL:            https://github.com/BarMy49/ai-gauge-rust
# winit and the tray load these at run time, so rpm cannot detect them.
Requires:       fontconfig
Requires:       libxkbcommon
Requires:       libxkbcommon-x11
Requires:       libX11
Requires:       libXcursor
Requires:       libXi
Requires:       libXrandr
Requires:       hicolor-icon-theme

%description
AI Gauge shows ChatGPT/Codex and Claude subscription limits and the
DeepSeek, OpenAI and Anthropic API balances in a window and the tray.

%install
install -Dm755 %{app_bin} %{buildroot}%{_bindir}/ai-gauge
install -Dm644 %{app_src}/assets/ai-gauge.desktop %{buildroot}%{_datadir}/applications/ai-gauge.desktop
install -Dm644 %{app_src}/assets/ai-gauge.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/ai-gauge.svg
install -Dm644 %{app_src}/LICENSE %{buildroot}%{_licensedir}/%{name}/LICENSE

%files
%license %{_licensedir}/%{name}/LICENSE
%{_bindir}/ai-gauge
%{_datadir}/applications/ai-gauge.desktop
%{_datadir}/icons/hicolor/scalable/apps/ai-gauge.svg
