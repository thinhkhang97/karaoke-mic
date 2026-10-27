//! OS-specific helpers: Bluetooth detection and microphone permission.
//! Only macOS has real implementations; other platforms get safe stubs.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MicPermission {
    Granted,
    Denied,
    Undetermined,
}

#[cfg(target_os = "macos")]
pub use mac::{is_bluetooth_output, mic_permission};

#[cfg(not(target_os = "macos"))]
pub fn is_bluetooth_output(_name: &str) -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn mic_permission() -> MicPermission {
    MicPermission::Granted
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{CStr, c_void};
    use std::mem::size_of;
    use std::ptr::null;

    use coreaudio_sys::{
        AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
        AudioObjectPropertyAddress, CFRelease, CFStringGetCString, CFStringRef,
        kAudioDevicePropertyTransportType, kAudioDeviceTransportTypeBluetooth,
        kAudioDeviceTransportTypeBluetoothLE, kAudioHardwarePropertyDevices,
        kAudioObjectPropertyElementMain, kAudioObjectPropertyName,
        kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, kCFStringEncodingUTF8,
    };
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    use super::MicPermission;

    const NAME_BUFFER: usize = 256;

    pub fn is_bluetooth_output(name: &str) -> bool {
        device_ids()
            .into_iter()
            .find(|&id| device_name(id).as_deref() == Some(name))
            .and_then(transport_type)
            .is_some_and(|t| {
                t == kAudioDeviceTransportTypeBluetooth || t == kAudioDeviceTransportTypeBluetoothLE
            })
    }

    pub fn mic_permission() -> MicPermission {
        let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
            return MicPermission::Undetermined;
        };
        match unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) } {
            AVAuthorizationStatus::Authorized => MicPermission::Granted,
            AVAuthorizationStatus::NotDetermined => MicPermission::Undetermined,
            _ => MicPermission::Denied,
        }
    }

    fn address(selector: u32) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress {
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        }
    }

    fn device_ids() -> Vec<AudioObjectID> {
        let addr = address(kAudioHardwarePropertyDevices);
        let mut size = 0u32;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &addr, 0, null(), &mut size)
        };
        if status != 0 || size == 0 {
            return Vec::new();
        }
        let mut ids = vec![0 as AudioObjectID; size as usize / size_of::<AudioObjectID>()];
        let status = unsafe {
            AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &addr,
                0,
                null(),
                &mut size,
                ids.as_mut_ptr() as *mut c_void,
            )
        };
        if status != 0 {
            return Vec::new();
        }
        ids
    }

    fn device_name(id: AudioObjectID) -> Option<String> {
        let addr = address(kAudioObjectPropertyName);
        let mut cf: CFStringRef = std::ptr::null();
        let mut size = size_of::<CFStringRef>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(id, &addr, 0, null(), &mut size, &mut cf as *mut _ as *mut c_void)
        };
        if status != 0 || cf.is_null() {
            return None;
        }
        let mut buf = [0 as std::os::raw::c_char; NAME_BUFFER];
        let ok = unsafe {
            CFStringGetCString(cf, buf.as_mut_ptr(), NAME_BUFFER as _, kCFStringEncodingUTF8)
        };
        unsafe { CFRelease(cf as _) };
        if ok == 0 {
            return None;
        }
        Some(unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned())
    }

    fn transport_type(id: AudioObjectID) -> Option<u32> {
        let addr = address(kAudioDevicePropertyTransportType);
        let mut value = 0u32;
        let mut size = size_of::<u32>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(id, &addr, 0, null(), &mut size, &mut value as *mut _ as *mut c_void)
        };
        (status == 0).then_some(value)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        #[ignore = "needs real audio hardware"]
        fn print_devices() {
            for id in device_ids() {
                let name = device_name(id).unwrap_or_default();
                println!("{name:40} bluetooth={}", is_bluetooth_output(&name));
            }
            println!("mic permission: {:?}", mic_permission());
        }
    }
}
