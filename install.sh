#!/usr/bin/env bash
# Build and install Maprisma Video Translator for the current user (no root needed):
#   ~/.local/bin/maprisma-video-translator-applet     the panel applet (subtitles icon)
#   ~/.local/bin/maprisma-video-translator-captions          the caption window (the applet keeps it running)
#   ~/.local/bin/maprisma-video-translator                   the Maprisma Video Translator window (Flutter)
#   ~/.local/share/maprisma-video-translator/backend/ the caption controller; "Set up now" in the menu
#                                                   installs its private runtime and the video-trans command
# Needs for building: cargo; for the window also flutter (FLUTTER, default /mnt/Data/flutter/bin/flutter),
# cmake and ninja.
set -euo pipefail
cd "$(dirname "$0")"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin"
flutter="${FLUTTER:-/mnt/Data/flutter/bin/flutter}"

if command -v cargo >/dev/null; then
  cargo build --release
elif [[ ! -x target/release/maprisma-video-translator-applet || ! -x target/release/maprisma-video-translator-captions ]]; then
  echo "cargo not found and prebuilt binaries are missing in target/release." >&2
  exit 1
fi
install -Dm755 target/release/maprisma-video-translator-applet "$bin/maprisma-video-translator-applet"
install -Dm755 target/release/maprisma-video-translator-captions "$bin/maprisma-video-translator-captions"

mkdir -p "$data/maprisma-video-translator"
rm -rf "$data/maprisma-video-translator/backend.new"
cp -r backend "$data/maprisma-video-translator/backend.new"
find "$data/maprisma-video-translator/backend.new" -name __pycache__ -prune -exec rm -rf {} +
rm -rf "$data/maprisma-video-translator/backend"
mv "$data/maprisma-video-translator/backend.new" "$data/maprisma-video-translator/backend"

if command -v cmake >/dev/null && command -v ninja >/dev/null && [[ -x $flutter ]]; then
  (cd app && "$flutter" build linux --release)
  rm -rf "$data/maprisma-video-translator/app"
  cp -r app/build/linux/x64/release/bundle "$data/maprisma-video-translator/app"
  ln -sf "$data/maprisma-video-translator/app/maprisma-video-translator" "$bin/maprisma-video-translator"
  mkdir -p "$data/applications"
  sed "s|^Exec=.*|Exec=$bin/maprisma-video-translator|" data/com.maprisma.VideoTranslator.desktop \
    > "$data/applications/com.maprisma.VideoTranslator.desktop"
else
  echo "Maprisma Video Translator window skipped: needs cmake, ninja and flutter ($flutter)." >&2
fi

# The panel starts applets without ~/.local/bin in PATH: the full path.
mkdir -p "$data/applications"
sed "s|^Exec=.*|Exec=$bin/maprisma-video-translator-applet|" data/io.github.ubruckhaus.CosmicMaprismaVideoTranslator.desktop \
  > "$data/applications/io.github.ubruckhaus.CosmicMaprismaVideoTranslator.desktop"
# An existing video-trans command follows the newly installed backend.
if [[ -f $bin/video-trans ]] && grep -qxF '# Managed by ubruckhaus.video-trans' "$bin/video-trans"; then
  bash "$data/maprisma-video-translator/backend/install-shortcuts.sh" >/dev/null
fi
echo "Installed. Add “Maprisma Video Translator” in COSMIC Settings › Desktop › Panel › Applets."
