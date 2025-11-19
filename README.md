# mic_toggle

A small Windows utility written in Rust to control the system microphone via **global hotkeys**. Runs in the background with a system tray icon, using the Windows CoreAudio API for system-level mute control.

## ✨ Features
- Works on **Windows 10 / 11**
- No admin rights required
- Operates via system-level mute (CoreAudio `IAudioEndpointVolume`)
- Tiny, fast, single `.exe` file
- **Runs in background by default** - No console window, just system tray icon
- **Global hotkey support** - Ctrl+Shift+Alt shortcuts for instant mute control
- **System tray icon** - Visual indicator with right-click menu to exit
- Perfect for always-on microphone control during calls, streaming, recording

## 🧰 Usage

### Default Mode (Background with Tray Icon)

Simply run the executable to start listening for hotkeys in the background:

```bash
mic_toggle.exe
```

**What happens:**
- ✅ Starts in background (no console window)
- ✅ Adds microphone icon to system tray
- ✅ Listens for global hotkeys
- ✅ Stays active until you exit from tray menu

**Global Hotkeys:**
- **Ctrl+Shift+Alt+M** - Toggle mute/unmute
- **Ctrl+Shift+Alt+I** - Mute microphone
- **Ctrl+Shift+Alt+U** - Unmute microphone
- **Ctrl+Shift+Alt+S** - Send HID command (custom device control)

**To Exit:**
- Right-click the tray icon and select "Exit"
- Or use Task Manager to terminate the process

### Console Mode (For Debugging)

Show the console window while listening:

```bash
mic_toggle.exe --console
```

- Shows startup messages and hotkey events
- Useful for troubleshooting
- Press Ctrl+C to exit

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