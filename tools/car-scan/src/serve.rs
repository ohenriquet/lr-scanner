//! Servidor local de ponte: serve o app Flutter (web) e expõe a leitura do
//! carro por HTTP (mesma origem, sem CORS). Permite testar a interface com
//! dados reais no navegador, sem precisar do build nativo/FFI.
//!
//! A sessão serial é usada sequencialmente (um pedido por vez), o que casa com
//! o barramento, que também é sequencial.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use diag::{CarSession, ModuleAddr, SerialTransport};
use serde_json::{json, Value};
use tiny_http::{Header, Method, Response, Server};

type Session = CarSession<SerialTransport>;

pub fn run(port: &str, baud: u32, web_dir: &str, http_port: u16) -> Result<()> {
    println!("Abrindo {port} @ {baud} baud...");
    let (session, status) = Session::open_serial(port, baud)
        .map_err(|e| anyhow::anyhow!("conectar ao Mongoose: {e}"))?;
    println!("Conectado (status {status:#010x}). Canal ISO-TP aberto.");

    let web = Arc::new(PathBuf::from(web_dir));
    anyhow::ensure!(
        web.join("index.html").is_file(),
        "web não encontrado em {web_dir} (rode: flutter build web)"
    );

    let addr = format!("127.0.0.1:{http_port}");
    let server = Arc::new(Server::http(&addr).map_err(|e| anyhow::anyhow!("abrir HTTP: {e}"))?);
    // A sessão de carro é serializada por um Mutex (o barramento é sequencial),
    // mas os arquivos estáticos e a UI são servidos por várias threads — assim a
    // página não congela enquanto uma leitura longa (scan/DTCs) está em curso.
    let session = Arc::new(Mutex::new(session));
    println!("App em http://{addr}/  (abra no navegador; Conectar já lê o carro)");

    let mut handles = Vec::new();
    for _ in 0..4 {
        let server = Arc::clone(&server);
        let session = Arc::clone(&session);
        let web = Arc::clone(&web);
        handles.push(thread::spawn(move || {
            while let Ok(mut req) = server.recv() {
                let url = req.url().to_string();
                let path = url.split('?').next().unwrap_or("/").to_string();
                if path.starts_with("/api/") {
                    let mut body = String::new();
                    let _ = std::io::Read::read_to_string(req.as_reader(), &mut body);
                    let method = req.method().clone();
                    let v = {
                        // recupera de um eventual mutex envenenado (não aborta o servidor)
                        let mut s = session.lock().unwrap_or_else(|p| p.into_inner());
                        handle_api(&mut s, &path, &method, &body)
                    };
                    let resp = Response::from_string(v.to_string()).with_header(json_header());
                    req.respond(resp).ok();
                } else {
                    serve_static(&web, &path, req);
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
}

fn handle_api(session: &mut Session, path: &str, method: &Method, body: &str) -> Value {
    match (path, method) {
        ("/api/connect", Method::Post) => json!({ "ok": true }),
        ("/api/vin", _) => match session.read_vin() {
            Some(vin) => json!({ "vin": vin }),
            None => json!({ "vin": Value::Null }),
        },
        ("/api/scan", Method::Post) => {
            let mods = parse_modules(body);
            let present = session.scan(&mods);
            let arr: Vec<Value> = present
                .iter()
                .map(|(n, req, resp)| json!([n, req, resp]))
                .collect();
            json!({ "present": arr })
        }
        ("/api/dtcs", Method::Post) => {
            let mods = parse_modules(body);
            let found = session.read_dtcs(&mods);
            let arr: Vec<Value> = found
                .iter()
                .map(|m| {
                    let d: Vec<Value> = m
                        .dtcs
                        .iter()
                        .map(|x| json!({ "code": x.code, "status": x.status }))
                        .collect();
                    json!({ "module": m.module, "dtcs": d })
                })
                .collect();
            json!({ "modules": arr })
        }
        ("/api/readmany", Method::Post) => {
            // body: [[req,resp,did], ...]
            let reqs: Vec<(u16, u16, u16)> = serde_json::from_str(body).unwrap_or_default();
            let mut out = serde_json::Map::new();
            for (req, resp, did) in reqs {
                let hex = session
                    .read_did_raw(req, resp, did)
                    .map(|b| b.iter().map(|x| format!("{x:02X}")).collect::<String>());
                out.insert(
                    format!("{req}:{resp}:{did}"),
                    hex.map(Value::String).unwrap_or(Value::Null),
                );
            }
            Value::Object(out)
        }
        _ => json!({ "error": "rota desconhecida" }),
    }
}

fn parse_modules(body: &str) -> Vec<ModuleAddr> {
    // O pack traz módulos repetidos (variantes de motor/ano com o mesmo
    // endereço CAN — p.ex. PCM/TCM ×3). Sondar o mesmo endereço várias vezes só
    // multiplica o tempo de scan, então deduplica por (req,resp).
    let v = serde_json::from_str::<Vec<(String, u16, u16)>>(body).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    v.into_iter()
        .filter(|(_, req, resp)| seen.insert((*req, *resp)))
        .collect()
}

fn serve_static(web: &Path, path: &str, req: tiny_http::Request) {
    let rel = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
    // evita escapar da pasta
    let safe = rel.replace("..", "");
    let file = web.join(&safe);
    match std::fs::read(&file) {
        Ok(bytes) => {
            let ct = content_type(&file);
            let resp = Response::from_data(bytes)
                .with_header(Header::from_bytes(&b"Content-Type"[..], ct.as_bytes()).unwrap());
            req.respond(resp).ok();
        }
        Err(_) => {
            req.respond(Response::from_string("not found").with_status_code(404))
                .ok();
        }
    }
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("css") => "text/css",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        _ => "application/octet-stream",
    }
}
