use anyhow::{Context, Result};
use std::env;
use windows::{
    core::Result as WinResult,
    Win32::{
        Foundation::BOOL,
        Media::Audio::{
            eCapture, eCommunications, eMultimedia, EDataFlow, ERole,
            IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
            Endpoints::IAudioEndpointVolume,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize,
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
            StructuredStorage::PROPVARIANT, // можно не использовать явно, но пусть будет импорт
        },
    },
};
use global_hotkey::{
    GlobalHotKeyManager, GlobalHotKeyEvent,
    hotkey::{HotKey, Code, Modifiers},
};

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
    eprintln!("Usage: mic_toggle [toggle|mute|unmute|status|--listen]");
    eprintln!("  toggle  - Toggle microphone mute state (default)");
    eprintln!("  mute    - Mute microphone");
    eprintln!("  unmute  - Unmute microphone");
    eprintln!("  status  - Show current mute status");
    eprintln!("  --listen - Stay running and listen for hotkeys:");
    eprintln!("             Ctrl+Shift+Alt+M - Toggle mute");
    eprintln!("             Ctrl+Shift+Alt+I - Mute");
    eprintln!("             Ctrl+Shift+Alt+U - Unmute");
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

fn listen_mode(epv: IAudioEndpointVolume) -> Result<()> {
    println!("Listen mode activated. Press Ctrl+C to exit.");
    println!("Hotkeys:");
    println!("  Ctrl+Shift+Alt+M - Toggle mute");
    println!("  Ctrl+Shift+Alt+I - Mute");
    println!("  Ctrl+Shift+Alt+U - Unmute");

    let manager = GlobalHotKeyManager::new()
        .context("Failed to create hotkey manager")?;

    // Define modifiers: Ctrl+Shift+Alt
    let modifiers = Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT;

    // Register hotkeys
    let hotkey_toggle = HotKey::new(Some(modifiers), Code::KeyM);
    let hotkey_mute = HotKey::new(Some(modifiers), Code::KeyI);
    let hotkey_unmute = HotKey::new(Some(modifiers), Code::KeyU);

    manager.register(hotkey_toggle)
        .context("Failed to register toggle hotkey (Ctrl+Shift+Alt+M)")?;
    manager.register(hotkey_mute)
        .context("Failed to register mute hotkey (Ctrl+Shift+Alt+I)")?;
    manager.register(hotkey_unmute)
        .context("Failed to register unmute hotkey (Ctrl+Shift+Alt+U)")?;

    println!("Hotkeys registered successfully. Listening...");

    // Event loop
    let receiver = GlobalHotKeyEvent::receiver();
    loop {
        if let Ok(event) = receiver.recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                let result = if event.id == hotkey_toggle.id() {
                    do_toggle(&epv)
                } else if event.id == hotkey_mute.id() {
                    do_mute(&epv)
                } else if event.id == hotkey_unmute.id() {
                    do_unmute(&epv)
                } else {
                    continue;
                };

                if let Err(e) = result {
                    eprintln!("Error: {}", e);
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let cmd = env::args().nth(1).unwrap_or_else(|| "toggle".to_string());
    let epv = get_endpoint_volume().context("Cannot get endpoint volume")?;

    let res = match cmd.as_str() {
        "--listen" | "-l" => {
            // Don't uninitialize COM here - keep it for listen mode
            return listen_mode(epv);
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
