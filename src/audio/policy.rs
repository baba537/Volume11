//! Changing the default playback device.
//!
//! Windows offers no public API for this. The Sound control panel, the quick
//! settings flyout and tools such as EarTrumpet and SoundSwitch all go through
//! `IPolicyConfig`, an undocumented COM interface that has kept the same shape
//! since Windows 7. It is declared here by hand.
//!
//! Only `SetDefaultEndpoint` is used, but a COM interface is a vtable: every
//! method before it has to be declared in order so that it lands in the right
//! slot. Their parameters are opaque pointers because nothing here calls them.

#![allow(non_snake_case)]

use std::ffi::c_void;

use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{ERole, eCommunications, eConsole, eMultimedia};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{GUID, HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

/// `CPolicyConfigClient`.
const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn GetMixFormat(&self, device: PCWSTR, format: *mut *mut c_void) -> HRESULT;
    fn GetDeviceFormat(&self, device: PCWSTR, default: i32, format: *mut *mut c_void) -> HRESULT;
    fn ResetDeviceFormat(&self, device: PCWSTR) -> HRESULT;
    fn SetDeviceFormat(&self, device: PCWSTR, endpoint: *mut c_void, mix: *mut c_void) -> HRESULT;
    fn GetProcessingPeriod(
        &self,
        device: PCWSTR,
        default: i32,
        default_period: *mut i64,
        minimum_period: *mut i64,
    ) -> HRESULT;
    fn SetProcessingPeriod(&self, device: PCWSTR, period: *mut i64) -> HRESULT;
    fn GetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
    fn SetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
    fn GetPropertyValue(
        &self,
        device: PCWSTR,
        key: *const PROPERTYKEY,
        value: *mut c_void,
    ) -> HRESULT;
    fn SetPropertyValue(
        &self,
        device: PCWSTR,
        key: *const PROPERTYKEY,
        value: *mut c_void,
    ) -> HRESULT;
    fn SetDefaultEndpoint(&self, device: PCWSTR, role: ERole) -> HRESULT;
    fn SetEndpointVisibility(&self, device: PCWSTR, visible: i32) -> HRESULT;
}

/// Make `device_id` the default playback device.
///
/// Set for all three roles, the way the Windows quick settings flyout does:
/// switching to headphones is expected to move games, media and calls together.
/// Must be called on a thread with COM initialised.
pub fn set_default_device(device_id: &str) -> windows::core::Result<()> {
    let id = HSTRING::from(device_id);

    unsafe {
        let policy: IPolicyConfig = CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL)?;

        for role in [eConsole, eMultimedia, eCommunications] {
            policy.SetDefaultEndpoint(PCWSTR(id.as_ptr()), role).ok()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Media::Audio::{
        DEVICE_STATE_ACTIVE, IMMDeviceEnumerator, MMDeviceEnumerator, eRender,
    };
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree};

    fn default_id(enumerator: &IMMDeviceEnumerator) -> String {
        unsafe {
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .unwrap();
            let raw = device.GetId().unwrap();
            let id = raw.to_string().unwrap();
            CoTaskMemFree(Some(raw.as_ptr() as *const _));
            id
        }
    }

    /// Switches the real default device away and back. Ignored by default
    /// because it changes the machine's audio output for a moment; run with
    /// `cargo test switch_and_restore -- --ignored` and optionally set
    /// `VOLUME11_TEST_TARGET` to the name fragment of a silent device.
    #[test]
    #[ignore]
    fn switch_and_restore() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).unwrap();

            let original = default_id(&enumerator);

            let hint = std::env::var("VOLUME11_TEST_TARGET").unwrap_or_default();
            let collection = enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
                .unwrap();

            let mut target = None;
            for index in 0..collection.GetCount().unwrap() {
                let device = collection.Item(index).unwrap();
                let raw = device.GetId().unwrap();
                let id = raw.to_string().unwrap();
                CoTaskMemFree(Some(raw.as_ptr() as *const _));

                let name = {
                    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
                    use windows::Win32::System::Com::STGM_READ;
                    let store = device.OpenPropertyStore(STGM_READ).unwrap();
                    store
                        .GetValue(&PKEY_Device_FriendlyName)
                        .unwrap()
                        .to_string()
                };

                if id != original && (hint.is_empty() || name.contains(&hint)) {
                    target = Some((id, name));
                    break;
                }
            }

            let (target, name) = target.expect("a second active playback device is needed");
            println!("switching to {name}");

            set_default_device(&target).unwrap();
            let switched = default_id(&enumerator);

            // Restore before asserting, so a failure never leaves the machine
            // on the wrong output.
            set_default_device(&original).unwrap();
            let restored = default_id(&enumerator);

            assert_eq!(switched, target, "default did not change");
            assert_eq!(restored, original, "default was not restored");
        }
    }
}
