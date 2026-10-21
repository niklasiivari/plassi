#!/usr/bin/env bash
# Package an already-built Linux x86_64 release binary.
set -euo pipefail
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
appdir="$work/Plassi.AppDir"
install -Dm755 target/release/plassi "$appdir/usr/bin/plassi"
install -Dm644 LICENSE "$appdir/usr/share/doc/plassi/LICENSE"
install -Dm644 THIRD_PARTY_NOTICES.md "$appdir/usr/share/doc/plassi/THIRD_PARTY_NOTICES.md"
install -Dm644 web/favicon.svg "$work/plassi.svg"

# Pin both the release and its bytes; continuous builds are mutable.
curl --fail --location --retry 3 \
  https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
  --output "$work/linuxdeploy.AppImage"
printf '%s  %s\n' c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d "$work/linuxdeploy.AppImage" | sha256sum --check
chmod +x "$work/linuxdeploy.AppImage"

# Fetch an immutable runtime asset explicitly; appimagetool's downloader does
# not reliably follow GitHub's download redirects.
curl --fail --location --retry 3 --header 'Accept: application/octet-stream' \
  https://api.github.com/repos/AppImage/type2-runtime/releases/assets/657374641 \
  --output "$work/runtime-x86_64"
printf '%s  %s\n' 617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46 "$work/runtime-x86_64" | sha256sum --check
export LDAI_RUNTIME_FILE="$work/runtime-x86_64"

# winit loads these at runtime, so dependency scanning alone cannot find them.
libraries=()
ldconfig=$(command -v ldconfig || echo /sbin/ldconfig)
for library in libX11.so.6 libXcursor.so.1 libXrandr.so.2 libXi.so.6 \
  libxkbcommon.so.0 libxkbcommon-x11.so.0 libwayland-client.so.0 \
  libwayland-cursor.so.0 libwayland-egl.so.1; do
  library_path=$("$ldconfig" -p | awk -v name="$library" '$1 == name && /x86-64/ && !found { print $NF; found=1 }')
  test -n "$library_path" || { echo "Missing runtime library: $library" >&2; exit 1; }
  libraries+=(--library "$library_path")
done
mkdir -p desktop-artifacts
export APPIMAGE_EXTRACT_AND_RUN=1
export ARCH=x86_64
export OUTPUT="$PWD/desktop-artifacts/Plassi-linux-x86_64.AppImage"
"$work/linuxdeploy.AppImage" --appdir "$appdir" \
  --executable "$appdir/usr/bin/plassi" \
  --desktop-file packaging/plassi.desktop --icon-file "$work/plassi.svg" \
  "${libraries[@]}" --output appimage
(cd desktop-artifacts && sha256sum Plassi-linux-x86_64.AppImage > SHA256SUMS.txt)
