# Maprisma Video Translator

Live translated captions over any video, call or microphone. Whisper recognises the speech on your computer. Your own llama.cpp or Ollama server, or an OpenAI-compatible online API, translates it. The captions appear in a movable, resizable window that stays above fullscreen video. German is the default target language.

One of the [Maprisma](https://github.com/maprisma) apps for COSMIC: the COSMIC version of the Omarchy plugin [omarchy-video-trans](https://github.com/uBruckhaus/omarchy-video-trans), which stays as it is. The caption controller and the speech worker are the same Python programs.

- **`applet/` (`maprisma-video-translator-applet`)**: the subtitles icon and its menu in the COSMIC panel (`libcosmic`). It keeps the caption window helper running.
- **`tray/` (`maprisma-video-translator-tray`)**: the KDE StatusNotifierItem tray icon and D-Bus menu, built with zbus. Shares backend calls and caption helper supervision with the COSMIC applet through `control/`.
- **`captions/` (`maprisma-video-translator-captions`)**: the caption window. It draws a transparent `wlr-layer-shell` surface on the overlay layer, which sits above all windows and fullscreen video. Only the caption box takes input.
- **`app/` (`maprisma-video-translator`)**: the Maprisma Video Translator window (Flutter), with the controls and all settings.
- **`backend/`**: the hidden caption controller (`gui.py`, Qt offscreen), the speech worker, first-use setup (`readiness.py`) and the JSON line bridge (`bridge.sh` / `bridge.py`). It is shared with the GNOME port.

## Install

```sh
./install.sh
```

This installs for the current user and needs no root:

- `~/.local/bin/maprisma-video-translator-applet`, `~/.local/bin/maprisma-video-translator-captions`, `~/.local/bin/maprisma-video-translator-tray`
- `~/.local/bin/maprisma-video-translator` (the bundle is in `~/.local/share/maprisma-video-translator/app/`)
- `~/.local/share/maprisma-video-translator/backend/`
- `~/.local/share/applications/io.github.ubruckhaus.CosmicMaprismaVideoTranslator.desktop` (applet) and `com.maprisma.VideoTranslator.desktop` (window)

Add **Maprisma Video Translator** in **COSMIC Settings → Desktop → Panel → Applets**. While something is missing, the menu says what, with a button that does it:

1. **Set up now** creates a private Python 3.12 runtime under `~/.local/share/video-trans/runtime`, with pinned PySide6, faster-whisper and httpx, plus the `video-trans` command. It needs `ffmpeg`, `pactl`, `uv` and (outside KDE) `grim`. Any that are missing are installed with `pacman`, after a password prompt.
2. **Use <device>** chooses the playback device to caption.
3. **Download Whisper …** fetches the speech model once (small: 480 MB).
4. If llama.cpp is not running and has no user service, the menu offers Ollama or an online provider.
5. **Choose a model automatically** picks an installed text model for the target language.

## Use

- **Subtitles icon in the panel.** Left click opens the menu: **Start**, **Stop**, **Captions**, the setup step, **Translate into**, **Listen to**, **Follow new captions**, the caption font size, **Clear captions** and **Open Maprisma Video Translator…**. Right click starts or stops captions. A green line under the icon shows that captions are running.
- **Maprisma Video Translator window** (`maprisma-video-translator`, also in the app launcher). It has the same controls plus all settings: Listening, Translation, Translation server, Captions, caption window appearance and keyboard shortcuts. It is single-instance: opening it again brings the open window forward.
- **Caption window.** Drag with the left mouse button to move it. Drag the bottom-right handle (shown while the pointer is over the captions) to resize it. Use the mouse wheel to read earlier captions. It covers the first output and shows on every workspace.
- **Keyboard shortcuts.** `video-trans <action>` controls a running session, for example `video-trans toggle`. Add keys for it in COSMIC Settings → Input Devices → Keyboard → Keyboard shortcuts → Custom shortcuts. `video-trans help` lists every action.

## KDE Plasma

KDE Plasma 6.8 on Wayland uses the **tray**, because the libcosmic panel applet requires COSMIC. Run `./install.sh` (builds the Rust workspace with `cargo build --release --offline`), then log in again, or start `~/.local/bin/maprisma-video-translator-tray` now. The installer adds `~/.config/autostart/com.maprisma.VideoTranslator.Tray.desktop` with `OnlyShowIn=KDE;` (honouring `XDG_CONFIG_HOME`). COSMIC continues to use its existing panel applet.

Left click opens the Maprisma Video Translator window; middle click starts or stops captions; right click opens the menu. It offers Start, Stop, Captions, guided setup, Clear captions and Settings…; the language, the audio input, following new captions and the caption font size are set in the settings window. Running captions use the tray's attention icon; the tooltip shows the backend status. The tray permits one instance per session bus and registers again after the Plasma tray watcher restarts.

The tray supervises the same captions helper as the applet, restarting it after exit. The helper exits when its parent dies. KWin's `zwlr_layer_shell_v1` supports the overlay above fullscreen video. KWin does not offer wlr-screencopy, so Automatic colour uses the default white text on a black backdrop on KDE, without invoking `grim`; setup does not require `grim`. On other desktops a failed capture disables sampling until the helper restarts. Keyboard shortcuts can be added in KDE System Settings using the full `~/.local/bin/video-trans` path expanded to your home directory; no virtual keyboard protocol is required.

For live verification: check login autostart, all three mouse buttons, menu settings and setup actions, running icon/tooltip, watcher restart, duplicate-instance rejection, helper restart and parent death. Check captions above fullscreen video, movement/resizing/scrolling and workspace switching. Confirm Automatic colour creates no repeated `grim` processes or errors on KDE. Also smoke-test the COSMIC applet after the shared-code extraction.

## Privacy

- Audio stays on your computer. Only recognised text goes to the translation server.
- **Automatic caption colour** samples the screen. Every 2 seconds, while the captions are open and Automatic is selected, the caption helper runs `grim` on the 60-pixel strip just above the captions, scaled to 5 % and held in memory. It sends only the mean brightness to the controller. Choose any other caption colour to stop this. KDE uses the fallback described above and does not sample the screen.
- The controller runs only while it is needed. If the caption helper stops polling (applet removed), the controller stops capturing within 10 seconds.

| Data | Location |
| --- | --- |
| Interface language | `~/.config/maprisma-video-translator/interface.json` (respects `XDG_CONFIG_HOME`) |
| Settings | `~/.config/video-trans/settings.json` |
| Saved API keys (optional, owner-only) | `~/.config/video-trans/credentials.json` |
| Python runtime | `~/.local/share/video-trans/runtime/` |
| Whisper models | `~/.cache/huggingface/hub/` |

## Interface languages

The Language choice at the top of the settings window offers English, Deutsch,
Français, Español, Italiano, Português, Nederlands and Polski. System follows the
first non-empty `LC_ALL`, `LC_MESSAGES`, or `LANG`; unsupported locales use English.
The selection is saved as `language: "auto"` or a language code in the interface
settings file. The window updates immediately; the applet, tray and caption status
update on their next refresh (normally within 1.5 seconds).

English text is the translation key: `tr('Start')` or `trf('System ({0})', [name])`.
Uncatalogued text stays English. Device/model names, paths, credential values and
caption content are data and stay unchanged. The shared Python backend is unchanged;
its human-readable messages are translated at the display boundary. Diagnostics
from external programs and providers fall back to their original wording.

There is no Omarchy video catalog. Matching translations in `translations/*.json`
reuse the Playground catalogs as terminology guidance; the app-specific text is in
`translations/extra/*.json`. English catalogs are empty because English is the key.
After editing a catalog, run `tools/gen-translations` to regenerate the checked-in
Dart source and Rust JSON include. Neither runtime needs loose files or downloads.

## Tests

```sh
tools/test-translations                      # source keys, all 7 catalogs, placeholders, desktop entries
/mnt/Data/flutter/bin/cache/dart-sdk/bin/dart app/test/i18n_check.dart
cargo build --offline
cargo test --offline                         # applet, tray menu/actions and caption helper
cd app && /mnt/Data/flutter/bin/flutter test && /mnt/Data/flutter/bin/flutter analyze
cd backend && python3 -m unittest tests.test_readiness tests.test_bridge
~/.local/share/video-trans/runtime/bin/python -m unittest discover -s tests   # all, after setup
```

`tools/fake_controller.py` serves fixed captions on the controller socket, so the caption window can be tried without the real controller.

MIT license.
