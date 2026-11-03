//! Ponte C-ABI entre o app Flutter (dart:ffi) e o núcleo de diagnóstico.
//!
//! Protocolo simples: cada função recebe/retorna strings JSON (C `char*`). As
//! strings retornadas são alocadas aqui e devem ser liberadas com
//! [`jlr_free`]. Uma sessão global (uma por processo) guarda a conexão serial
//! com o Mongoose.
//!
//! Funções:
//! - `jlr_version() -> {version, connected}`
//! - `jlr_connect(port, baud) -> {ok, status?, error?}`
//! - `jlr_vin() -> {vin?, error?}`
//! - `jlr_scan(modules_json) -> {present:[[sigla,req,resp]], error?}`
//! - `jlr_dtcs(present_json) -> {modules:[{module, dtcs:[{code,status}]}], error?}`
//! - `jlr_read_did(req, resp, did) -> {hex?, error?}`
//! - `jlr_disconnect()`
//! - `jlr_free(ptr)`

use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

use diag::{CarSession, ModuleAddr, ModuleDtcs, SerialTransport};
#[cfg(target_os = "android")]
use diag::UsbFdTransport;
use serde_json::{json, Value};

/// Sessão ativa: serial (desktop) ou USB-OTG por fd (Android). Os métodos de
/// diagnóstico são genéricos em `CarSession<T>`; aqui só despachamos para o
/// transporte certo.
enum AnySession {
    Serial(CarSession<SerialTransport>),
    #[cfg(target_os = "android")]
    Usb(CarSession<UsbFdTransport>),
}

impl AnySession {
    fn read_vin(&mut self) -> Option<String> {
        match self {
            Self::Serial(s) => s.read_vin(),
            #[cfg(target_os = "android")]
            Self::Usb(s) => s.read_vin(),
        }
    }
    fn scan(&mut self, m: &[ModuleAddr]) -> Vec<ModuleAddr> {
        match self {
            Self::Serial(s) => s.scan(m),
            #[cfg(target_os = "android")]
            Self::Usb(s) => s.scan(m),
        }
    }
    fn read_dtcs(&mut self, m: &[ModuleAddr]) -> Vec<ModuleDtcs> {
        match self {
            Self::Serial(s) => s.read_dtcs(m),
            #[cfg(target_os = "android")]
            Self::Usb(s) => s.read_dtcs(m),
        }
    }
    fn read_did_raw(&mut self, req: u16, resp: u16, did: u16) -> Option<Vec<u8>> {
        match self {
            Self::Serial(s) => s.read_did_raw(req, resp, did),
            #[cfg(target_os = "android")]
            Self::Usb(s) => s.read_did_raw(req, resp, did),
        }
    }
}

type Session = AnySession;

fn session() -> &'static Mutex<Option<Session>> {
    static S: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Converte um JSON em `char*` alocado (liberar com `jlr_free`).
fn to_c(v: Value) -> *mut c_char {
    CString::new(v.to_string())
        .unwrap_or_else(|_| CString::new("{}").unwrap())
        .into_raw()
}

fn err(msg: impl std::fmt::Display) -> *mut c_char {
    to_c(json!({ "error": msg.to_string() }))
}

/// Lê um `char*` de entrada como `&str` (vazio se nulo/inválido).
unsafe fn in_str<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        return "";
    }
    CStr::from_ptr(p).to_str().unwrap_or("")
}

/// Executa `f` protegido contra panics; em panic devolve JSON de erro.
fn guard(f: impl FnOnce() -> *mut c_char) -> *mut c_char {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(p) => p,
        Err(_) => err("panic interno na ponte"),
    }
}

fn with_session(f: impl FnOnce(&mut Session) -> *mut c_char) -> *mut c_char {
    let mut g = match session().lock() {
        Ok(g) => g,
        Err(_) => return err("sessão travada"),
    };
    match g.as_mut() {
        Some(s) => f(s),
        None => err("não conectado"),
    }
}

/// Versão da ponte e se há sessão ativa.
#[no_mangle]
pub extern "C" fn jlr_version() -> *mut c_char {
    guard(|| {
        let connected = session().lock().map(|g| g.is_some()).unwrap_or(false);
        to_c(json!({ "version": env!("CARGO_PKG_VERSION"), "connected": connected }))
    })
}

/// Conecta à porta serial do Mongoose e abre o canal ISO-TP.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)] // fronteira FFI: ponteiros vindos do Dart
pub extern "C" fn jlr_connect(port: *const c_char, baud: c_int) -> *mut c_char {
    guard(|| {
        let port = unsafe { in_str(port) }.to_string();
        let baud = if baud > 0 { baud as u32 } else { 500_000 };
        match CarSession::<SerialTransport>::open_serial(&port, baud) {
            Ok((s, status)) => {
                if let Ok(mut g) = session().lock() {
                    *g = Some(AnySession::Serial(s));
                }
                to_c(json!({ "ok": true, "status": status }))
            }
            Err(e) => to_c(json!({ "ok": false, "error": e })),
        }
    })
}

/// Conecta pelo fd do `UsbManager` (Android USB-OTG) e abre o canal ISO-TP.
/// EXPERIMENTAL — ver `crates/diag/src/usb_fd.rs`.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn jlr_connect_fd(fd: c_int, baud: c_int) -> *mut c_char {
    guard(|| {
        let baud = if baud > 0 { baud as u32 } else { 500_000 };
        match CarSession::<UsbFdTransport>::open_usb_fd(fd as std::os::fd::RawFd, baud) {
            Ok((s, status)) => {
                if let Ok(mut g) = session().lock() {
                    *g = Some(AnySession::Usb(s));
                }
                to_c(json!({ "ok": true, "status": status }))
            }
            Err(e) => to_c(json!({ "ok": false, "error": e })),
        }
    })
}

/// Encerra a sessão (fecha a porta).
#[no_mangle]
pub extern "C" fn jlr_disconnect() {
    let _ = catch_unwind(|| {
        if let Ok(mut g) = session().lock() {
            *g = None;
        }
    });
}

/// Lê o VIN.
#[no_mangle]
pub extern "C" fn jlr_vin() -> *mut c_char {
    guard(|| {
        with_session(|s| match s.read_vin() {
            Some(vin) => to_c(json!({ "vin": vin })),
            None => to_c(json!({ "vin": Value::Null })),
        })
    })
}

/// Varre quais módulos respondem. `modules_json` = `[["PCM",2016,2024], ...]`.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn jlr_scan(modules_json: *const c_char) -> *mut c_char {
    guard(|| {
        let mods = match parse_modules(unsafe { in_str(modules_json) }) {
            Ok(m) => m,
            Err(e) => return err(e),
        };
        with_session(|s| {
            let present = s.scan(&mods);
            let arr: Vec<Value> = present
                .iter()
                .map(|(n, req, resp)| json!([n, req, resp]))
                .collect();
            to_c(json!({ "present": arr }))
        })
    })
}

/// Lê os DTCs dos módulos informados (`present_json` no mesmo formato do scan).
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn jlr_dtcs(present_json: *const c_char) -> *mut c_char {
    guard(|| {
        let mods = match parse_modules(unsafe { in_str(present_json) }) {
            Ok(m) => m,
            Err(e) => return err(e),
        };
        with_session(|s| {
            let found = s.read_dtcs(&mods);
            let arr: Vec<Value> = found
                .iter()
                .map(|m| {
                    let dtcs: Vec<Value> = m
                        .dtcs
                        .iter()
                        .map(|d| json!({ "code": d.code, "status": d.status }))
                        .collect();
                    json!({ "module": m.module, "dtcs": dtcs })
                })
                .collect();
            to_c(json!({ "modules": arr }))
        })
    })
}

/// Lê um DID cru (UDS 0x22). Devolve os bytes de dado em hex.
#[no_mangle]
pub extern "C" fn jlr_read_did(req: c_int, resp: c_int, did: c_int) -> *mut c_char {
    guard(|| {
        with_session(|s| {
            match s.read_did_raw(req as u16, resp as u16, did as u16) {
                Some(bytes) => {
                    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
                    to_c(json!({ "hex": hex }))
                }
                None => to_c(json!({ "hex": Value::Null })),
            }
        })
    })
}

/// Libera uma string devolvida por esta ponte.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn jlr_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// `[["PCM",2016,2024], ...]` -> Vec<(String,u16,u16)>.
fn parse_modules(s: &str) -> Result<Vec<ModuleAddr>, String> {
    if s.trim().is_empty() {
        return Ok(Vec::new());
    }
    let raw: Vec<(String, u16, u16)> =
        serde_json::from_str(s).map_err(|e| format!("módulos JSON inválido: {e}"))?;
    Ok(raw)
}
