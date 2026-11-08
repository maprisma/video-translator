# Maprisma Video Translator

Live translated captions over any video, call or microphone. Whisper recognises the speech on your computer. Your own llama.cpp or Ollama server, or an OpenAI-compatible online API, translates it. The captions appear in a movable, resizable window that stays above fullscreen video. German is the default target language.

One of the [Maprisma](https://github.com/maprisma) apps for COSMIC: the COSMIC version of the Omarchy plugin [omarchy-video-trans](https://github.com/uBruckhaus/omarchy-video-trans), which stays as it is. The caption controller and the speech worker are the same Python programs.

- **`applet/` (`maprisma-video-translator-applet`)**: the subtitles icon and its menu in the COSMIC panel (`libcosmic`). It keeps the caption window helper running.
- **`captions/` (`maprisma-video-translator-captions`)**: the caption window. It draws a transparent `wlr-layer-shell` surface on the overlay layer, which sits above all windows and fullscreen video. Only the caption box takes input.
- **`app/` (`maprisma-video-translator`)**: the Maprisma Video Translator window (Flutter), with the controls and all settings.
- **`backend/`**: the hidden caption controller (`gui.py`, Qt offscreen), the speech worker, first-use setup (`readiness.py`) and the JSON line bridge (`bridge.sh` / `bridge.py`). It is shared with the GNOME port.

## Install

```sh
./install.sh
```

This installs for the current user and needs no root:

- `~/.local/bin/maprisma-video-translator-applet`, `~/.local/bin/maprisma-video-translator-captions`
- `~/.local/bin/maprisma-video-translator` (the bundle is in `~/.local/share/maprisma-video-translator/app/`)
- `~/.local/share/maprisma-video-translator/backend/`
- `~/.local/share/applications/io.github.ubruckhaus.CosmicMaprismaVideoTranslator.desktop` (applet) and `com.maprisma.VideoTranslator.desktop` (window)

Add **Maprisma Video Translator** in **COSMIC Settings → Desktop → Panel → Applets**. While something is missing, the menu says what, with a button that does it:

1. **Set up now** creates a private Python 3.12 runtime under `~/.local/share/video-trans/runtime`, with pinned PySide6, faster-whisper and httpx, plus the `video-trans` command. It needs `ffmpeg`, `pactl`, `uv` and `grim`. Any that are missing are installed with `pacman`, after a password prompt.
2. **Use <device>** chooses the playback device to caption.
3. **Download Whisper …** fetches the speech model once (small: 480 MB).
4. If llama.cpp is not running and has no user service, the menu offers Ollama or an online provider.
5. **Choose a model automatically** picks an installed text model for the target language.

## Use

- **Subtitles icon in the panel.** Left click opens the menu: **Start**, **Stop**, **Captions**, the setup step, **Translate into**, **Listen to**, **Follow new captions**, the caption font size, **Clear captions** and **Open Maprisma Video Translator…**. Right click starts or stops captions. A green line under the icon shows that captions are running.
- **Maprisma Video Translator window** (`maprisma-video-translator`, also in the app launcher). It has the same controls plus all settings: Listening, Translation, Translation server, Captions, caption window appearance and keyboard shortcuts. It is single-instance: opening it again brings the open window forward.
- **Caption window.** Drag with the left mouse button to move it. Drag the bottom-right handle (shown while the pointer is over the captions) to resize it. Use the mouse wheel to read earlier captions. It covers the first output and shows on every workspace.
- **Keyboard shortcuts.** `video-trans <action>` controls a running session, for example `video-trans toggle`. Add keys for it in COSMIC Settings → Input Devices → Keyboard → Keyboard shortcuts → Custom shortcuts. `video-trans help` lists every action.

## Privacy

- Audio stays on your computer. Only recognised text goes to the translation server.
- **Automatic caption colour** samples the screen. Every 2 seconds, while the captions are open and Automatic is selected, the caption helper runs `grim` on the 60-pixel strip just above the captions, scaled to 5 % and held in memory. It sends only the mean brightness to the controller. Choose any other caption colour to stop this.
- The controller runs only while it is needed. If the caption helper stops polling (applet removed), the controller stops capturing within 10 seconds.

| Data | Location |
| --- | --- |
| Settings | `~/.config/video-trans/settings.json` |
| Saved API keys (optional, owner-only) | `~/.config/video-trans/credentials.json` |
| Python runtime | `~/.local/share/video-trans/runtime/` |
| Whisper models | `~/.cache/huggingface/hub/` |

## Tests

```sh
cargo test                                   # applet and caption helper
cd app && /mnt/Data/flutter/bin/flutter test && /mnt/Data/flutter/bin/flutter analyze
cd backend && python3 -m unittest tests.test_readiness tests.test_bridge
~/.local/share/video-trans/runtime/bin/python -m unittest discover -s tests   # all, after setup
```

`tools/fake_controller.py` serves fixed captions on the controller socket, so the caption window can be tried without the real controller.

MIT license.
