# mic_toggle

A small Windows utility written in Rust to **mute / unmute / toggle / check** the system microphone using the Windows CoreAudio API.

## ✨ Features
- Works on **Windows 10 / 11**
- No admin rights required
- Operates via system-level mute (CoreAudio `IAudioEndpointVolume`)
- Tiny, fast, single `.exe` file
- **Global hotkey support** - Listen mode with Ctrl+Shift+Alt shortcuts
- **System tray icon** - Visual indicator with right-click menu to exit
- **Silent background mode** - Run without console window
- Useful for scripts, automation, or hardware macro buttons

## 🧰 Commands

```bash
mic_toggle.exe status   # Show current mute state
mic_toggle.exe mute     # Mute microphone
mic_toggle.exe unmute   # Unmute microphone
mic_toggle.exe toggle   # Toggle mute/unmute (default)
mic_toggle.exe --listen # Stay running and listen for global hotkeys
```

### Listen Mode

The `--listen` flag keeps the application running in the background and listens for global keyboard shortcuts:

- **Ctrl+Shift+Alt+M** - Toggle mute/unmute
- **Ctrl+Shift+Alt+I** - Mute microphone
- **Ctrl+Shift+Alt+U** - Unmute microphone
- **Ctrl+Shift+Alt+S** - Send HID command (custom device control)

This is useful for always-on microphone control without needing to bind the executable to external macro software.

```bash
mic_toggle.exe --listen
# or
mic_toggle.exe -l

# Run in background without console window
mic_toggle.exe --listen --silent
```

Press Ctrl+C to exit listen mode (when not running in silent mode).

### System Tray Icon

When running in listen mode, a system tray icon appears showing that the application is active. Right-click the icon to access the menu:
- **Exit** - Close the application gracefully

### Silent Mode

Use the `--silent` flag with `--listen` to spawn a detached background process without a console window. The command will return immediately after starting the background process.

**Note:** When running in silent mode, you can exit the application by right-clicking the system tray icon and selecting "Exit", or by using Task Manager to terminate the `mic_toggle.exe` process.

## 🧩 Build Instructions

### 1️⃣ Install Rust
Install Rust using the official installer:  
👉 [https://rustup.rs](https://rustup.rs)

####️⃣ Install Visual Studio Build Tools (MSVC)

Rust on Windows requires the MSVC toolchain.

Download the installer:
👉 https://aka.ms/vs/17/release/vs_buildtools.exe

During setup:

- ✅ Select Desktop development with C++

- ✅ Check MSVC v143 build tools (or newer)

- ✅ Check Windows 10/11 SDK

- Then install and restart the terminal (or open x64 Native Tools Command Prompt for VS).