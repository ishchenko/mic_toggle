use anyhow::{Context, Result};
use std::env;
use std::thread;
use std::time::Duration;
use std::process::Command;
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
    eprintln!("Usage: mic_toggle [toggle|mute|unmute|status|--listen [--silent]]");
    eprintln!("  toggle  - Toggle microphone mute state (default)");
    eprintln!("  mute    - Mute microphone");
    eprintln!("  unmute  - Unmute microphone");
    eprintln!("  status  - Show current mute status");
    eprintln!("  --listen - Stay running and listen for hotkeys:");
    eprintln!("             Ctrl+Shift+Alt+M - Toggle mute");
    eprintln!("             Ctrl+Shift+Alt+I - Mute");
    eprintln!("             Ctrl+Shift+Alt+U - Unmute");
    eprintln!("             Ctrl+Shift+Alt+S - Send HID command");
    eprintln!("  --silent - Use with --listen to hide console window");
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

fn listen_mode(epv: IAudioEndpointVolume, background: bool) -> Result<()> {
    if !background {
        println!("Listen mode activated. Press Ctrl+C to exit.");
        println!("Hotkeys:");
        println!("  Ctrl+Shift+Alt+M - Toggle mute");
        println!("  Ctrl+Shift+Alt+I - Mute");
        println!("  Ctrl+Shift+Alt+U - Unmute");
        println!("  Ctrl+Shift+Alt+S - Send HID command");
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

    // Event loop with Windows message pump
    let receiver = GlobalHotKeyEvent::receiver();
    loop {
        // Process Windows messages (required for global hotkeys to work on Windows)
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageA(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageA(&msg);
            }
        }

        // Check for hotkey events
        if let Ok(event) = receiver.try_recv() {
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

        // Small sleep to prevent busy-waiting
        thread::sleep(Duration::from_millis(10));
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("toggle");

    // Check for --silent flag (can be anywhere in args)
    let silent = args.iter().any(|arg| arg == "--silent");

    // Check for internal __background flag (used when spawning detached process)
    let is_background = args.iter().any(|arg| arg == "__background");

    // If --silent is specified and we're NOT already the background process,
    // spawn a detached process and exit
    if silent && !is_background && (cmd == "--listen" || cmd == "-l") {
        let exe_path = env::current_exe()
            .context("Failed to get current executable path")?;

        // Spawn detached process without console window
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            Command::new(exe_path)
                .arg("--listen")
                .arg("__background")
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .context("Failed to spawn background process")?;

            println!("Background process started.");
            return Ok(());
        }

        #[cfg(not(windows))]
        {
            eprintln!("--silent mode is only supported on Windows");
            return Ok(());
        }
    }

    let epv = get_endpoint_volume().context("Cannot get endpoint volume")?;

    let res = match cmd {
        "--listen" | "-l" => {
            // Don't uninitialize COM here - keep it for listen mode
            return listen_mode(epv, is_background);
        }
        "status" => {
            unsafe {
                let muted: BOOL = epv.GetMute()?;
                println!("Muted: {}", muted.as_bool());
            }
            Ok(())
        }
        "mute" => do_mute(&epv),
        "unmute" => do_unmute(&epv),
        "toggle" => do_toggle(&epv),
        _ => {
            print_usage();
            Ok(())
        }
    };

    unsafe { CoUninitialize(); }
    res
}
