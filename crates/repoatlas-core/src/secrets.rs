use crate::error::{Error, Result};
use std::env;

#[cfg(target_os = "macos")]
const MACOS_SERVICE: &str = "io.repoatlas.desktop.provider";

#[cfg(windows)]
mod windows_dpapi {
    use super::{Error, Result};
    use std::ffi::c_void;
    use std::ptr;

    #[repr(C)]
    struct DataBlob {
        data_len: u32,
        data: *mut u8,
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *const DataBlob,
            data_descr: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt_struct: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const DataBlob,
            data_descr: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt_struct: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    pub fn protect(plain: &str) -> Result<Vec<u8>> {
        let mut input = plain.as_bytes().to_vec();
        let input_blob = DataBlob {
            data_len: input.len() as u32,
            data: input.as_mut_ptr(),
        };
        let mut output = DataBlob {
            data_len: 0,
            data: ptr::null_mut(),
        };
        let ok = unsafe {
            CryptProtectData(
                &input_blob,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(Error::msg(
                "could not encrypt API key for this Windows user",
            ));
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(output.data, output.data_len as usize).to_vec() };
        unsafe {
            LocalFree(output.data as *mut c_void);
        }
        Ok(bytes)
    }

    pub fn unprotect(blob: &[u8]) -> Result<String> {
        let mut input = blob.to_vec();
        let input_blob = DataBlob {
            data_len: input.len() as u32,
            data: input.as_mut_ptr(),
        };
        let mut output = DataBlob {
            data_len: 0,
            data: ptr::null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &input_blob,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(Error::msg("could not decrypt the saved API key"));
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(output.data, output.data_len as usize).to_vec() };
        unsafe {
            LocalFree(output.data as *mut c_void);
        }
        String::from_utf8(bytes).map_err(|_| Error::msg("saved API key is not valid UTF-8"))
    }
}

#[cfg(target_os = "macos")]
fn macos_account(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

#[cfg(target_os = "macos")]
fn macos_store(provider_id: &str, secret: &str) -> Result<()> {
    let status = std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            MACOS_SERVICE,
            "-a",
            &macos_account(provider_id),
            "-w",
            secret,
        ])
        .status()
        .map_err(|error| Error::msg(format!("macOS Keychain is unavailable: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::msg("could not store the API key in macOS Keychain"))
    }
}

#[cfg(target_os = "macos")]
fn macos_load(provider_id: &str) -> Result<Option<String>> {
    let output = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            MACOS_SERVICE,
            "-a",
            &macos_account(provider_id),
            "-w",
        ])
        .output()
        .map_err(|error| Error::msg(format!("macOS Keychain is unavailable: {error}")))?;
    if !output.status.success() {
        return Ok(None);
    }
    let secret = String::from_utf8(output.stdout)
        .map_err(|_| Error::msg("saved API key is not valid UTF-8"))?;
    let secret = secret.trim_end_matches(['\n', '\r']).to_string();
    if secret.is_empty() {
        Ok(None)
    } else {
        Ok(Some(secret))
    }
}

#[cfg(target_os = "macos")]
fn macos_delete(provider_id: &str) -> Result<()> {
    let _ = std::process::Command::new("security")
        .args([
            "delete-generic-password",
            "-s",
            MACOS_SERVICE,
            "-a",
            &macos_account(provider_id),
        ])
        .status();
    Ok(())
}

pub fn store_secret(provider_id: &str, secret: &str) -> Result<Option<Vec<u8>>> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Ok(None);
    }
    #[cfg(windows)]
    {
        let _ = provider_id;
        Ok(Some(windows_dpapi::protect(secret)?))
    }
    #[cfg(target_os = "macos")]
    {
        macos_store(provider_id, secret)?;
        return Ok(None);
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = provider_id;
        Err(Error::msg(
            "encrypted API key storage is only available on Windows and macOS",
        ))
    }
}

pub fn load_secret(provider_id: &str, blob: Option<&[u8]>) -> Result<String> {
    #[cfg(windows)]
    {
        let _ = provider_id;
        let Some(blob) = blob.filter(|value| !value.is_empty()) else {
            return Ok(String::new());
        };
        windows_dpapi::unprotect(blob)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = blob;
        return Ok(macos_load(provider_id)?.unwrap_or_default());
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (provider_id, blob);
        Ok(String::new())
    }
}

pub fn delete_secret(provider_id: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos_delete(provider_id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = provider_id;
        Ok(())
    }
}

pub fn looks_like_env_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Legacy profiles stored an environment variable name in `credential_ref`.
/// Keep those working until the user saves a real API key.
pub fn resolve_legacy_env(credential_ref: &str) -> String {
    let name = credential_ref.trim();
    if name.is_empty() || !looks_like_env_name(name) {
        return String::new();
    }
    env::var(name).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_dpapi_round_trips_an_api_key() {
        let blob = store_secret("provider-test", "sk-test-secret").unwrap();
        let restored = load_secret("provider-test", blob.as_deref()).unwrap();
        assert_eq!(restored, "sk-test-secret");
    }

    #[test]
    fn empty_secret_is_not_stored() {
        assert_eq!(store_secret("provider-test", "   ").unwrap(), None);
    }
}
