Found two concrete issues:

- **[P2] A transient capture failure permanently changes COSMIC behaviour** — [captions/src/main.rs:169](/mnt/Data/Projects/maprisma-video-translator/captions/src/main.rs:169). Any failed `grim` invocation or three-second timeout sets `CAPTURE_FAILED` for the helper’s lifetime. For example, a capture failure during an output reconfiguration leaves Automatic colour permanently using white-on-black, even after capture works again. Previously, sampling retried every two seconds. The shared-module extraction preserves behaviour, but this separate change violates the requirement that COSMIC remain unchanged.

- **[P2] Fresh installations cannot fetch build dependencies** — [install.sh:17](/mnt/Data/Projects/maprisma-video-translator/install.sh:17). Adding unconditional `--offline` makes the documented `./install.sh` fail when the user’s Cargo cache lacks any dependency, including the newly added zbus version. The repository does not vendor dependencies. A fresh checkout with Cargo installed and internet available therefore fails before installation; previously Cargo fetched what was missing.

The requested KWin backend, long-lived watcher script, ScreenShot2 handling, fake input, and screenshot desktop-entry authorization **are absent from this checkout**. The README instead explicitly disables screen sampling on KDE. Those paths could not be reviewed.

The tray’s exported D-Bus methods perform no caller authorization: any peer permitted to reach it on the session bus can invoke activation, toggle captions, or dispatch enabled menu actions, including setup actions. This is a broader entry point than the owner-only controller socket; there is no restriction to Plasma’s tray host.

The extracted control functions, including helper supervision, retain their original logic. `PDEATHSIG` handling is unchanged. Four focused Python tests passed; broader tests were blocked by sandbox restrictions on temporary files and sockets. No files were modified.