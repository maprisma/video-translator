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

- **Subtitles icon in the panel.** Left click opens the menu: **Start**, **Stop**, **Captions**, the setup step, **Translate into**, **Listen to**, **Follow new captions**, the caption font size, **Clear captions** and **Open Maprisma Video Translator…**. A second click or **Esc** closes it (Esc closes an open drop-down first). Right click starts or stops captions. A green line under the icon shows that captions are running.
- **Closing and size.** **Esc** in the Maprisma Video Translator window closes an open drop-down or dialog first, then the window (as its close button: captions on screen go on). `maprisma-video-translator --toggle` opens the window, or closes it when it is shown, focused or not (the KDE/Hyprland tray's click; a bar click takes the focus first, the Maprisma rule). The window keeps the size you give it (`window.json` next to `interface.json`; not while maximised, fullscreen or tiled).
- **Maprisma Video Translator window** (`maprisma-video-translator`, also in the app launcher). It has the same controls plus all settings: Listening, Translation, Translation server, Captions, caption window appearance and keyboard shortcuts. It is single-instance: opening it again brings the open window forward.
- **Caption window.** Drag with the left mouse button to move it. Drag the bottom-right handle (shown while the pointer is over the captions) to resize it. Use the mouse wheel to read earlier captions. It covers the first output and shows on every workspace.
- **Keyboard shortcuts.** `video-trans <action>` controls a running session, for example `video-trans toggle`. Add keys for it in COSMIC Settings → Input Devices → Keyboard → Keyboard shortcuts → Custom shortcuts; on KDE in System Settings → Keyboard → Shortcuts → Add New → Command or Script (with the full path `~/.local/bin/video-trans`, expanded); on Hyprland with a key binding. Each key press runs the command once (no hold/release). `video-trans help` lists every action.

## KDE Plasma

KDE Plasma 6.8 and Hyprland use the **tray**, because the libcosmic panel applet requires COSMIC. Run `./install.sh` (builds the Rust workspace with `cargo build --release --offline`), then log in again, or start `~/.local/bin/maprisma-video-translator-tray` now. The installer adds `~/.config/autostart/com.maprisma.VideoTranslator.Tray.desktop` with `OnlyShowIn=KDE;Hyprland;` (honouring `XDG_CONFIG_HOME`). COSMIC continues to use its existing panel applet.

Left click opens the Maprisma Video Translator window; middle click starts or stops captions; right click opens the menu. It offers Start, Stop, Captions, guided setup, Clear captions and Settings…; the language, the audio input, following new captions and the caption font size are set in the settings window. The subtitles icon is filled in the theme green while captions run (`-active-symbolic`, also the attention icon) and a plain outline otherwise; the tooltip shows the backend status. The tray permits one instance per session bus and registers again after the Plasma tray watcher restarts.

The tray supervises the same captions helper as the applet, restarting it after exit. The helper exits when its parent dies. KWin's `zwlr_layer_shell_v1` supports the overlay above fullscreen video. KWin does not offer wlr-screencopy, so Automatic colour uses the default white text on a black backdrop on KDE, without invoking `grim`; setup does not require `grim`. On other desktops a failed capture disables sampling until the helper restarts. Keyboard shortcuts can be added in KDE System Settings using the full `~/.local/bin/video-trans` path expanded to your home directory; no virtual keyboard protocol is required.

For live verification: check login autostart, all three mouse buttons, menu settings and setup actions, running icon/tooltip, watcher restart, duplicate-instance rejection, helper restart and parent death. Check captions above fullscreen video, movement/resizing/scrolling and workspace switching. Confirm Automatic colour creates no repeated `grim` processes or errors on KDE. Also smoke-test the COSMIC applet after the shared-code extraction.

## Privacy

- Audio stays on your computer. Only recognised text goes to the translation server.
- **Automatic caption colour** samples the screen. Every 2 seconds, while the captions are open and Automatic is selected, the caption helper runs `grim` on the 60-pixel strip just above the captions, scaled to 5 % and held in memory. It sends only the mean brightness to the controller. Choose any other caption colour to stop this. KDE uses the fallback described above and does not sample the screen.
- The controller runs only while it is needed. If the caption helper stops polling (applet removed), the controller stops capturing within 10 seconds.

| Data | Location |
| --- | --- |
| Interface language, F12 and Corners settings | `~/.config/maprisma-video-translator/interface.json` (respects `XDG_CONFIG_HOME`) |
| F12 screenshots and notes (only when you press F12) | `~/Pictures/Maprisma Video Translator/` |
| Settings | `~/.config/video-trans/settings.json` |
| Saved API keys (optional, owner-only) | `~/.config/video-trans/credentials.json` |
| Python runtime | `~/.local/share/video-trans/runtime/` |
| Whisper models | `~/.cache/huggingface/hub/` |

## F12: a screenshot with a note

While the panel popup or the Maprisma Video Translator window has the keyboard, **F12** (without Ctrl, Shift, Alt or Super) saves a picture of it, for telling what is wrong with it, as in the other Maprisma apps:

- **Only on F12, and only that popup or window.** The popup is drawn once more by iced into the picture (`window::screenshot`, encoded as PNG in the applet), the window by Flutter (`RepaintBoundary.toImage`, dialogs included). No screenshot program runs (the caption colour sampling with `grim` is not involved), nothing else on the screen is in it, and nothing is uploaded or sent anywhere. Not captured: the caption window (an overlay that never takes the keyboard, `KeyboardInteractivity::None`, so F12 cannot reach it) and the KDE tray's menu (drawn by Plasma).
- **Where:** `~/Pictures/Maprisma Video Translator/maprisma-video-translator-<date>-<time>.png`. The pictures folder comes from `~/.config/user-dirs.dirs` when it is set there (read only as a regular file of at most 64 KiB, never through a link). The app then names the file and asks **What is wrong? (optional)**; **Enter** or **Done** saves the text next to the picture as a `.txt` with the same name (at most 4000 characters; no text, no file), **Esc** skips it, and closing the popup keeps what was typed. F12 waits until that is closed.
- **Private files:** the folder is made `0700` when missing and only used when it is a real folder of yours (not a link) that no one else can write to, checked on the opened folder; pictures and notes are new `0600` files made inside that folder, never through a link and never over an existing file (`-2`, `-3` … are added instead, also when only a note has the name), synced to disk, and removed again when they cannot be written completely.
- **Off:** switch **F12 saves a screenshot** off in the window (next to Language); stored as `"screenshot_key": false` in `interface.json` (below); the applet picks it up on its next refresh.

## Corners

**Corners** in the window (next to Language): *Like the desktop (N px)* (the default: the COSMIC theme exactly as it is, the native look; N is the theme's medium corner radius, 16, 8 or 2 px by COSMIC Settings › Appearance, 16 px where there is no COSMIC theme, e.g. on KDE), *Slightly rounded (6 px)*, *Rounded (12 px)* or *Square*, as in the other Maprisma COSMIC apps. With one of the last three, popups, cards, the note card and drop-down menus take that radius; drop-downs, text fields and menu rows at most 8 px; small parts (bars and their tracks, slider tracks, check boxes) at most 4 px; pills (libcosmic's buttons and toggles, round dots) stay round, or square in *Square*. In the applet's popup: libcosmic's widgets take their corners from the theme, so the popup's content is drawn with a copy of the active theme with these corner radii (iced's `themer`); slider knobs take the full radius. The panel button keeps the panel's look. In the window, *Like the desktop* keeps Material's own corners; otherwise cards and dialogs take the radius, buttons and choice buttons at most 8 px, check boxes at most 4 px; Material's switches stay pills, and the borderless text fields have no corners. The caption window keeps its own look. Stored as `"corners"` (`desktop`, `slight`, `rounded`, `square`) in `interface.json`; a file without it stays valid and means *Like the desktop*.

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
