# mic_toggle

A small Windows utility written in Rust to **mute / unmute / toggle / check** the system microphone using the Windows CoreAudio API.

## ✨ Features
- Works on **Windows 10 / 11**
- No admin rights required
- Operates via system-level mute (CoreAudio `IAudioEndpointVolume`)
- Tiny, fast, single `.exe` file
- **Runs in background by default** - No console window, just system tray icon
- **Global hotkey support** - Ctrl+Shift+Alt shortcuts for instant mute control
- **System tray icon** - Visual indicator with right-click menu to exit
- **One-time actions** - Can also perform single toggle/mute/unmute and exit
- Useful for always-on microphone control, scripts, and automation

## 🧰 Usage

### Default Mode (Background Listener)

By default, `mic_toggle` runs in the background with a system tray icon and listens for global hotkeys:

```bash
mic_toggle.exe              # Runs in background, shows tray icon
```

**Global Hotkeys:**
- **Ctrl+Shift+Alt+M** - Toggle mute/unmute
- **Ctrl+Shift+Alt+I** - Mute microphone
- **Ctrl+Shift+Alt+U** - Unmute microphone
- **Ctrl+Shift+Alt+S** - Send HID command (custom device control)

**System Tray:**
- A microphone icon appears in your system tray
- Right-click the icon and select "Exit" to close the application

### Console Mode

Show the console window while listening (useful for debugging):

```bash
mic_toggle.exe --console    # Show console, listen for hotkeys
```

Press Ctrl+C to exit.

### One-Time Actions

Perform a single action and exit (without staying in background):

```bash
mic_toggle.exe --once status   # Show current mute state
mic_toggle.exe --once mute     # Mute microphone
mic_toggle.exe --once unmute   # Unmute microphone
mic_toggle.exe --once toggle   # Toggle mute/unmute
```

You can combine with `--console` to see output:

```bash
mic_toggle.exe --once status --console
```

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