use std::{ffi::OsString, os::windows::ffi::OsStringExt};

use log::{error, trace};
use windows::{
    Win32::Foundation::{CloseHandle, MAX_PATH},
    Win32::Media::Audio::Endpoints::IAudioEndpointVolume,
    Win32::Media::Audio::{
        IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
        MMDeviceEnumerator, eConsole, eRender,
    },
    Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
    Win32::System::ProcessStatus::K32GetModuleBaseNameW,
    Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    core::{Interface, Result as WindowsResult},
};

pub fn windows_init() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        if let Err(e) = CoInitializeEx(None, COINIT_MULTITHREADED).ok() {
            error!("Failed to initialize COM: {}", e);
            return Err(Box::new(e));
        }
    }
    Ok(())
}

pub fn set_master_volume(volume: f64) {
    unsafe {
        let enumerator: WindowsResult<IMMDeviceEnumerator> =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL);

        if let Ok(enumerator) = enumerator {
            if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                let endpoint_vol: WindowsResult<IAudioEndpointVolume> =
                    device.Activate(CLSCTX_ALL, None);

                if let Ok(endpoint_vol) = endpoint_vol {
                    let _ =
                        endpoint_vol.SetMasterVolumeLevelScalar(volume as f32, std::ptr::null());
                    trace!("Set master volume to {}", volume);
                }
            }
        }
    }
}

pub fn set_current_app_volume(volume: f64) {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid == 0 {
            return;
        }

        with_session_enumerator(|session_enum, count| {
            for i in 0..count {
                if let Ok(control) = session_enum.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        if let Ok(session_pid) = control2.GetProcessId() {
                            if session_pid == pid {
                                // Match found! Set volume.
                                if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                                    let _ =
                                        simple_vol.SetMasterVolume(volume as f32, std::ptr::null());
                                    trace!("Set focused app (PID {}) volume to {}", pid, volume);
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

pub fn set_app_volume(target_app_name: &str, volume: f64) {
    let target_lower = target_app_name.to_lowercase();

    unsafe {
        with_session_enumerator(|session_enum, count| {
            for i in 0..count {
                if let Ok(control) = session_enum.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        if let Ok(pid) = control2.GetProcessId() {
                            if let Some(name) = get_process_name(pid) {
                                if name.to_lowercase().contains(&target_lower) {
                                    if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                                        let _ = simple_vol
                                            .SetMasterVolume(volume as f32, std::ptr::null());
                                        trace!("Set {} volume to {}", name, volume);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

pub fn set_unmapped_volume(volume: f64, mapped_apps: &Vec<String>) {
    let excluded_lower: Vec<String> = mapped_apps.iter().map(|s| s.to_lowercase()).collect();
    unsafe {
        with_session_enumerator(|session_enum, count| {
            for i in 0..count {
                if let Ok(control) = session_enum.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        if let Ok(pid) = control2.GetProcessId() {
                            if let Some(name) = get_process_name(pid) {
                                let name_lower = name.to_lowercase();

                                // Check if this process name is in the excluded list
                                let mut is_excluded = false;
                                for excluded in &excluded_lower {
                                    if name_lower.contains(excluded) {
                                        is_excluded = true;
                                        break;
                                    }
                                }

                                // Only set volume if NOT excluded
                                if !is_excluded {
                                    if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                                        let _ = simple_vol
                                            .SetMasterVolume(volume as f32, std::ptr::null());
                                        trace!("Set unmapped app {} volume to {}", name, volume);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

unsafe fn with_session_enumerator<F>(mut callback: F)
where
    F: FnMut(&windows::Win32::Media::Audio::IAudioSessionEnumerator, i32),
{
    unsafe {
        let enumerator: WindowsResult<IMMDeviceEnumerator> =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL);

        if let Ok(enumerator) = enumerator {
            if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                let manager: WindowsResult<IAudioSessionManager2> =
                    device.Activate(CLSCTX_ALL, None);

                if let Ok(manager) = manager {
                    if let Ok(session_enum) = manager.GetSessionEnumerator() {
                        if let Ok(count) = session_enum.GetCount() {
                            callback(&session_enum, count);
                        }
                    }
                }
            }
        }
    }
}

unsafe fn get_process_name(process_id: u32) -> Option<String> {
    if process_id == 0 {
        return None;
    }

    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            process_id,
        )
        .ok()?;

        if handle.is_invalid() {
            return None;
        }

        let mut buffer = [0u16; MAX_PATH as usize];
        let result = K32GetModuleBaseNameW(handle, None, &mut buffer);
        let _ = CloseHandle(handle);

        if result == 0 {
            return None;
        }

        let len = result as usize;
        let name = OsString::from_wide(&buffer[0..len])
            .to_string_lossy()
            .into_owned();

        Some(name)
    }
}
