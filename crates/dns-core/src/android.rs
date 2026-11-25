use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde::{Deserialize, Serialize};

use crate::catalog::Catalog;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidPrivateDnsGuide {
    pub server_id: String,
    pub server_name: String,
    pub private_dns_hostname: Option<String>,
    pub ipv4: Vec<String>,
    pub doh_url: Option<String>,
    pub setup_steps: Vec<String>,
}

pub fn build_android_guide(catalog: &Catalog, server_id: &str) -> Option<AndroidPrivateDnsGuide> {
    let srv = catalog.find_by_id(server_id)?;
    let steps = if let Some(ref dot) = srv.dot {
        vec![
            "Open Settings -> Network & internet -> Private DNS.".to_string(),
            "Select 'Private DNS provider hostname'.".to_string(),
            format!("Enter: {dot}"),
            "Tap Save. Android encrypts all system DNS traffic via DNS-over-TLS (port 853).".to_string(),
        ]
    } else {
        vec![
            "This server does not publish a DNS-over-TLS hostname for Android Private DNS.".to_string(),
            format!("Use static Wi-Fi DNS settings with IPv4: {}", srv.ipv4.join(", ")),
        ]
    };

    Some(AndroidPrivateDnsGuide {
        server_id: srv.id.clone(),
        server_name: srv.name.clone(),
        private_dns_hostname: srv.dot.clone(),
        ipv4: srv.ipv4.clone(),
        doh_url: srv.doh.clone(),
        setup_steps: steps,
    })
}

/// C-ABI export for Android JNI / Kotlin / Swift callers to inspect the embedded catalog JSON.
#[no_mangle]
pub extern "C" fn dns_optimizer_catalog_json() -> *mut c_char {
    let out = match Catalog::load_embedded() {
        Ok(c) => serde_json::to_string(&c.servers).unwrap_or_else(|_| "[]".to_string()),
        Err(_) => "[]".to_string(),
    };
    CString::new(out)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// C-ABI export returning the Android Private DNS hostname for a server ID.
///
/// # Safety
/// `server_id` must be a valid NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn dns_optimizer_android_dot_hostname(server_id: *const c_char) -> *mut c_char {
    if server_id.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(id_str) = CStr::from_ptr(server_id).to_str() else {
        return std::ptr::null_mut();
    };
    let Ok(catalog) = Catalog::load_embedded() else {
        return std::ptr::null_mut();
    };
    let Some(srv) = catalog.find_by_id(id_str) else {
        return std::ptr::null_mut();
    };
    let Some(dot) = srv.dot.as_deref() else {
        return std::ptr::null_mut();
    };
    CString::new(dot)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Free a C string previously returned by `dns_optimizer_*` FFI functions.
///
/// # Safety
/// `ptr` must be a pointer returned by a `dns_optimizer_*` FFI function or null.
#[no_mangle]
pub unsafe extern "C" fn dns_optimizer_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}
