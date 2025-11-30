use anyhow::{Result, anyhow};
use log::{error, trace};
use std::{ffi::OsString, os::windows::ffi::OsStringExt};
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

pub fn windows_init() -> Result<()> {
    unsafe {
        if let Err(e) = CoInitializeEx(None, COINIT_MULTITHREADED).ok() {
            error!("Failed to initialize COM: {}", e);
            return Err(e.into());
        }
    }
    Ok(())
}

pub fn set_master_volume(volume: f64) -> Result<()> {
    unsafe {
        let enumerator: WindowsResult<IMMDeviceEnumerator> =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL);

        if let Ok(enumerator) = enumerator {
            if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                let endpoint_vol: WindowsResult<IAudioEndpointVolume> =
                    device.Activate(CLSCTX_ALL, None);

                if let Ok(endpoint_vol) = endpoint_vol {
                    endpoint_vol.SetMute(volume <= 0.0, std::ptr::null())?;
                    endpoint_vol.SetMasterVolumeLevelScalar(volume as f32, std::ptr::null())?;
                    trace!("Set master volume to {}", volume);
                }
            }
        }
        Ok(())
    }
}

pub fn set_current_app_volume(volume: f64) -> Result<()> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Ok(());
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid == 0 {
            return Ok(());
        }

        with_session_enumerator(|session_enum, count| {
            for i in 0..count {
                let process_session = || -> Result<()> {
                    let control = session_enum.GetSession(i)?;
                    let control2 = control.cast::<IAudioSessionControl2>()?;
                    let session_pid = control2.GetProcessId()?;

                    if session_pid == pid {
                        let simple_vol = control.cast::<ISimpleAudioVolume>()?;
                        set_volume(simple_vol, volume)?;
                        trace!("Set focused app (PID {}) volume to {}", pid, volume);
                    }
                    Ok(())
                };

                let _ = process_session();
            }
            Ok(())
        })?;
        Ok(())
    }
}

pub fn set_app_volume(target_app_name: &str, volume: f64) -> Result<()> {
    let target_lower = target_app_name.to_lowercase();

    unsafe {
        with_session_enumerator(|session_enum, count| {
            for i in 0..count {
                let process_session = || -> Result<()> {
                    let control = session_enum.GetSession(i)?;
                    let control2 = control.cast::<IAudioSessionControl2>()?;
                    let pid = control2.GetProcessId()?;

                    let name =
                        get_process_name(pid).ok_or_else(|| anyhow!("Process name not found"))?;

                    if name.to_lowercase().contains(&target_lower) {
                        let simple_vol = control.cast::<ISimpleAudioVolume>()?;
                        set_volume(simple_vol, volume)?;
                        trace!("Set {} volume to {}", name, volume);
                    }
                    Ok(())
                };

                let _ = process_session();
            }
            Ok(())
        })?;
        Ok(())
    }
}

pub fn set_unmapped_volume(volume: f64, mapped_apps: &Vec<String>) -> Result<()> {
    let excluded_lower: Vec<String> = mapped_apps.iter().map(|s| s.to_lowercase()).collect();

    unsafe {
        with_session_enumerator(|session_enum, count| {
            for i in 0..count {
                let process_session = || -> Result<()> {
                    let control = session_enum.GetSession(i)?;
                    let control2 = control.cast::<IAudioSessionControl2>()?;
                    let pid = control2.GetProcessId()?;

                    let name =
                        get_process_name(pid).ok_or_else(|| anyhow!("Process name not found"))?;

                    let name_lower = name.to_lowercase();

                    let is_excluded = excluded_lower.iter().any(|ex| name_lower.contains(ex));

                    if !is_excluded {
                        let simple_vol = control.cast::<ISimpleAudioVolume>()?;
                        set_volume(simple_vol, volume)?;
                        trace!("Set unmapped app {} volume to {}", name, volume);
                    }
                    Ok(())
                };

                let _ = process_session();
            }
            Ok(())
        })?;
        Ok(())
    }
}

unsafe fn set_volume(sav: ISimpleAudioVolume, volume: f64) -> Result<()> {
    let volume = volume.clamp(0.0, 1.0);
    unsafe { sav.SetMute(volume <= 0.0, std::ptr::null())? }
    unsafe { sav.SetMasterVolume(volume as f32, std::ptr::null())? }
    Ok(())
}

unsafe fn with_session_enumerator<F>(mut callback: F) -> Result<()>
where
    F: FnMut(&windows::Win32::Media::Audio::IAudioSessionEnumerator, i32) -> Result<()>,
{
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;

        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;

        let session_enum = manager.GetSessionEnumerator()?;

        let count = session_enum.GetCount()?;

        callback(&session_enum, count)?;

        Ok(())
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
