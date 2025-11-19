use anyhow::{Context, Result};
use std::env;
use std::thread;
use std::time::{Duration, Instant};
use std::process::Command;
use std::io::Write;
use windows::{
    core::Result as WinResult,
    Win32::{
        Foundation::{BOOL, HWND},
        Media::Audio::{
            eCapture, eCommunications, eMultimedia, EDataFlow, ERole,
            IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
            Endpoints::IAudioEndpointVolume,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize,
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        },
        UI::WindowsAndMessaging::{
            PeekMessageA, TranslateMessage, DispatchMessageA,
            MSG, PM_REMOVE,
        },
    },
};
use global_hotkey::{
    GlobalHotKeyManager, GlobalHotKeyEvent,
    hotkey::{HotKey, Code, Modifiers},
};
use hidapi::HidApi;
use tray_icon::{
    TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
    Icon,
};
use serialport::SerialPort;

// Create a simple microphone icon (16x16 pixels)
fn create_tray_icon() -> Icon {
    let width = 16;
    let height = 16;
    let mut rgba = vec![0u8; width * height * 4];

    // Create a simple microphone shape (white on transparent)
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;

            // Microphone body (center vertical line)
            if x >= 6 && x <= 9 && y >= 3 && y <= 9 {
                rgba[idx] = 255;     // R
                rgba[idx + 1] = 255; // G
                rgba[idx + 2] = 255; // B
                rgba[idx + 3] = 255; // A
            }
            // Microphone top (rounded)
            else if y >= 2 && y <= 3 && x >= 7 && x <= 8 {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
            // Microphone stand
            else if y >= 10 && y <= 12 && x >= 7 && x <= 8 {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
            // Base
            else if y >= 12 && y <= 13 && x >= 5 && x <= 10 {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
        }
    }

    Icon::from_rgba(rgba, width as u32, height as u32).expect("Failed to create icon")
}

fn get_endpoint_volume() -> Result<IAudioEndpointVolume> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .context("CoInitializeEx failed")?;
    }

    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
            .context("CoCreateInstance(IMMDeviceEnumerator) failed")?
    };

    fn activate_endpoint(enumerator: &IMMDeviceEnumerator, role: ERole) -> WinResult<IAudioEndpointVolume> {
        unsafe {
            let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(EDataFlow(eCapture.0), role)?;
            // В 0.48 метод существует, когда включена фича StructuredStorage (PROPVARIANT).
            device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None)
        }
    }

    let epv = activate_endpoint(&enumerator, ERole(eCommunications.0))
        .or_else(|_| activate_endpoint(&enumerator, ERole(eMultimedia.0)))
        .context("Failed to get IAudioEndpointVolume for default capture device")?;

    Ok(epv)
}

fn print_usage() {
    eprintln!("Usage: mic_toggle [--console] [--port <COM_PORT>]");
    eprintln!();
    eprintln!("Runs in background with system tray icon and listens for global hotkeys.");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --console        Show console window (instead of running in background)");
    eprintln!("  --port <name>    Send mic state to COM port (e.g., --port COM10)");
    eprintln!("                   Writes '1' when unmuted, '0' when muted, every second");
    eprintln!();
    eprintln!("Hotkeys:");
    eprintln!("  Ctrl+Shift+Alt+M - Toggle mute");
    eprintln!("  Ctrl+Shift+Alt+I - Mute");
    eprintln!("  Ctrl+Shift+Alt+U - Unmute");
    eprintln!("  Ctrl+Shift+Alt+S - Send HID command");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  mic_toggle                Run in background with tray icon");
    eprintln!("  mic_toggle --console      Show console while listening (Ctrl+C to exit)");
    eprintln!("  mic_toggle --port COM10   Report mic state to COM10 every second");
}

fn do_mute(epv: &IAudioEndpointVolume) -> Result<()> {
    unsafe {
        epv.SetMute(BOOL(1), std::ptr::null())?;
    }
    println!("Muted.");
    Ok(())
}

fn do_unmute(epv: &IAudioEndpointVolume) -> Result<()> {
    unsafe {
        epv.SetMute(BOOL(0), std::ptr::null())?;
    }
    println!("Unmuted.");
    Ok(())
}

fn do_toggle(epv: &IAudioEndpointVolume) -> Result<()> {
    unsafe {
        let muted: BOOL = epv.GetMute()?;
        let new_state = if muted.as_bool() { BOOL(0) } else { BOOL(1) };
        epv.SetMute(new_state, std::ptr::null())?;
        println!("{}", if muted.as_bool() { "Unmuted." } else { "Muted." });
    }
    Ok(())
}

fn do_hid_command() -> Result<()> {
    // VID/PID for the HID device
    const VID: u16 = 0x0bda;
    const PID: u16 = 0x1100;

    // HID report buffer: 193 bytes. [0] — Report ID (0 in this case)
    let mut buf = [0u8; 193];

    // buf[1:3] = [0x40, 0xc6]
    buf[1] = 0x40;
    buf[2] = 0xC6;

    // buf[7:12] = [0x20, 0x00, 0x6e, 0x00, 0x80]
    buf[7]  = 0x20;
    buf[8]  = 0x00;
    buf[9]  = 0x6E;
    buf[10] = 0x00;
    buf[11] = 0x80;

    // msg = struct.pack(">HBB", 0xe069, 0, 1)
    // Big-endian u16 + two u8
    let mut msg = [0u8; 4]; // 2 + 1 + 1
    let e069 = 0xE069u16.to_be_bytes();
    msg[0] = e069[0];
    msg[1] = e069[1];
    msg[2] = 0x00;
    msg[3] = 0x01;

    // buf[65:68] = [0x51, 0x81 + len(msg), 0x03]
    // len(msg) == 4
    buf[65] = 0x51;
    buf[66] = 0x81 + (msg.len() as u8);
    buf[67] = 0x03;

    // buf[68:68+len(msg)] = msg
    buf[68..68 + msg.len()].copy_from_slice(&msg);

    // Initialize HID and write
    let api = HidApi::new().context("init hidapi")?;
    let device = api
        .open(VID, PID)
        .with_context(|| format!("open device {:04x}:{:04x}", VID, PID))?;

    // hidapi::HidDevice::write expects 0th byte to be Report ID (0 if not used)
    let written = device
        .write(&buf)
        .context("hid write failed")?;

    println!("HID command sent ({} bytes)", written);
    Ok(())
}

fn listen_mode(epv: IAudioEndpointVolume, background: bool, port_name: Option<String>) -> Result<()> {
    if !background {
        println!("Listen mode activated. Press Ctrl+C to exit.");
        println!("Hotkeys:");
        println!("  Ctrl+Shift+Alt+M - Toggle mute");
        println!("  Ctrl+Shift+Alt+I - Mute");
        println!("  Ctrl+Shift+Alt+U - Unmute");
        println!("  Ctrl+Shift+Alt+S - Send HID command");
    }

    // Initialize COM port if specified
    let mut serial_port: Option<Box<dyn SerialPort>> = None;
    if let Some(ref port) = port_name {
        match serialport::new(port, 9600)
            .timeout(Duration::from_millis(100))
            .open()
        {
            Ok(port) => {
                serial_port = Some(port);
                if !background {
                    println!("Connected to {}", port_name.as_ref().unwrap());
                }
            }
            Err(e) => {
                eprintln!("Error: Failed to open port {}: {}", port, e);
                eprintln!("COM port communication will not be available.");
            }
        }
    }

    let manager = GlobalHotKeyManager::new()
        .context("Failed to create hotkey manager")?;

    // Define modifiers: Ctrl+Shift+Alt
    let modifiers = Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT;

    // Register hotkeys
    let hotkey_toggle = HotKey::new(Some(modifiers), Code::KeyM);
    let hotkey_mute = HotKey::new(Some(modifiers), Code::KeyI);
    let hotkey_unmute = HotKey::new(Some(modifiers), Code::KeyU);
    let hotkey_hid = HotKey::new(Some(modifiers), Code::KeyS);

    manager.register(hotkey_toggle)
        .context("Failed to register toggle hotkey (Ctrl+Shift+Alt+M)")?;
    manager.register(hotkey_mute)
        .context("Failed to register mute hotkey (Ctrl+Shift+Alt+I)")?;
    manager.register(hotkey_unmute)
        .context("Failed to register unmute hotkey (Ctrl+Shift+Alt+U)")?;
    manager.register(hotkey_hid)
        .context("Failed to register HID hotkey (Ctrl+Shift+Alt+S)")?;

    if !background {
        println!("Hotkeys registered successfully. Listening...");
    }

    // Create system tray icon with menu
    let tray_menu = Menu::new();
    let exit_item = MenuItem::new("Exit", true, None);
    tray_menu.append(&exit_item)
        .context("Failed to add Exit menu item")?;

    let icon = create_tray_icon();
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Mic Toggle - Listening for hotkeys")
        .with_icon(icon)
        .build()
        .context("Failed to create system tray icon")?;

    // Event loop with Windows message pump
    let hotkey_receiver = GlobalHotKeyEvent::receiver();
    let menu_receiver = MenuEvent::receiver();
    let mut last_port_write = Instant::now();

    loop {
        // Process Windows messages (required for global hotkeys to work on Windows)
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageA(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageA(&msg);
            }
        }

        // Check for menu events (Exit clicked)
        if let Ok(event) = menu_receiver.try_recv() {
            if event.id == exit_item.id() {
                if !background {
                    println!("Exiting...");
                }
                break;
            }
        }

        // Check for hotkey events
        if let Ok(event) = hotkey_receiver.try_recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                let result = if event.id == hotkey_toggle.id() {
                    do_toggle(&epv)
                } else if event.id == hotkey_mute.id() {
                    do_mute(&epv)
                } else if event.id == hotkey_unmute.id() {
                    do_unmute(&epv)
                } else if event.id == hotkey_hid.id() {
                    do_hid_command()
                } else {
                    continue;
                };

                if let Err(e) = result {
                    eprintln!("Error: {}", e);
                }
            }
        }

        // Write mic state to COM port every second
        if port_name.is_some() && last_port_write.elapsed() >= Duration::from_secs(1) {
            last_port_write = Instant::now();

            // Get current mic state
            let is_muted = unsafe {
                epv.GetMute().unwrap_or(BOOL(0)).as_bool()
            };

            // Write '1' if unmuted (mic is on), '0' if muted (mic is off)
            let state_byte = if is_muted { b'0' } else { b'1' };

            // Try to write to port, attempt reconnection if failed
            if let Some(ref mut port) = serial_port {
                if let Err(e) = port.write_all(&[state_byte]) {
                    if !background {
                        eprintln!("COM port write error: {}. Will retry...", e);
                    }
                    // Drop the failed port, try to reconnect next time
                    serial_port = None;
                }
            }

            // Try to reconnect if port is None
            if serial_port.is_none() && port_name.is_some() {
                if let Some(ref port_str) = port_name {
                    match serialport::new(port_str, 9600)
                        .timeout(Duration::from_millis(100))
                        .open()
                    {
                        Ok(port) => {
                            serial_port = Some(port);
                            if !background {
                                println!("Reconnected to {}", port_str);
                            }
                            // Try to write the state immediately after reconnection
                            if let Some(ref mut port) = serial_port {
                                let _ = port.write_all(&[state_byte]);
                            }
                        }
                        Err(_) => {
                            // Silently fail, will retry next second
                        }
                    }
                }
            }
        }

        // Small sleep to prevent busy-waiting
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // Check for flags (can be anywhere in args)
    let show_console = args.iter().any(|arg| arg == "--console");
    let is_background = args.iter().any(|arg| arg == "__background");

    // Parse --port parameter
    let port_name = args.iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .cloned();

    // If NOT --console and NOT already background, spawn background process
    if !show_console && !is_background {
        let exe_path = env::current_exe()
            .context("Failed to get current executable path")?;

        // Spawn detached process without console window
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            let mut cmd = Command::new(exe_path);
            cmd.arg("__background");

            // Pass --port parameter to background process if specified
            if let Some(ref port) = port_name {
                cmd.arg("--port").arg(port);
            }

            cmd.creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .context("Failed to spawn background process")?;

            println!("Background process started. Check system tray for icon.");
            return Ok(());
        }

        #[cfg(not(windows))]
        {
            eprintln!("Background mode is only supported on Windows");
            return Ok(());
        }
    }

    // Run listen mode (either in foreground with --console, or as background process)
    let epv = get_endpoint_volume().context("Cannot get endpoint volume")?;
    listen_mode(epv, is_background, port_name)
}
