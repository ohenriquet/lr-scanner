//! Diagnóstico do veículo sobre o Mongoose: VIN, presença de módulos, DTCs,
//! leitura crua de DIDs e limpeza de DTCs. Reaproveitado pelo `car-scan`
//! (futuro) e pela ponte FFI do app (`bridge`).
//!
//! Tudo aqui é LEITURA, exceto [`CarSession::clear_dtcs`] (UDS 0x14), que apaga
//! a memória de falhas dos módulos — o app só chama com confirmação do usuário.
//!
//! Os endereços de módulo NÃO são fixos aqui: o chamador passa a lista
//! `(sigla, req, resp)` derivada do pacote do veículo, mantendo isto genérico.

use std::io::{Read, Write};
use std::time::Duration;

use mongoose::client::{Device, IsoTpResponse};
use mongoose::Transport;

#[cfg(target_os = "android")]
mod usb_fd;
#[cfg(target_os = "android")]
pub use usb_fd::UsbFdTransport;

/// Um módulo endereçável: sigla, id de requisição (tx) e de resposta (rx).
pub type ModuleAddr = (String, u16, u16);

/// Transporte serial (porta COM do Mongoose no desktop; CDC-ACM no Linux/macOS).
pub struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
}

impl SerialTransport {
    pub fn open(name: &str, baud: u32) -> Result<Self, String> {
        let mut port = serialport::new(name, baud)
            .timeout(Duration::from_millis(60))
            .open()
            .map_err(|e| format!("abrir {name}: {e}"))?;
        let _ = port.write_data_terminal_ready(true);
        let _ = port.write_request_to_send(true);
        Ok(Self { port })
    }
}

impl Transport for SerialTransport {
    type Error = std::io::Error;
    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.port.write_all(data)?;
        self.port.flush()
    }
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }
}

/// Converte os 2 primeiros bytes de um DTC no código base (ex.: `C1A13`).
pub fn dtc_base_code(b0: u8, b1: u8) -> String {
    let letter = match b0 >> 6 {
        0 => 'P',
        1 => 'C',
        2 => 'B',
        _ => 'U',
    };
    let d1 = (b0 >> 4) & 0x3;
    let d2 = b0 & 0x0F;
    format!("{letter}{d1}{d2:X}{b1:02X}")
}

/// Código DTC completo com tipo de falha (ex.: `C1A13-08`); `00` = sem sufixo.
pub fn dtc_code(b0: u8, b1: u8, b2: u8) -> String {
    let base = dtc_base_code(b0, b1);
    if b2 == 0 {
        base
    } else {
        format!("{base}-{b2:02X}")
    }
}

/// Decodifica a resposta `59 02 <mask> [3 bytes + status]*` em (3 bytes, status).
pub fn decode_dtcs(service: &[u8]) -> Vec<([u8; 3], u8)> {
    let mut out = Vec::new();
    if service.len() < 3 || service[0] != 0x59 || service[1] != 0x02 {
        return out;
    }
    let mut i = 3;
    while i + 4 <= service.len() {
        out.push(([service[i], service[i + 1], service[i + 2]], service[i + 3]));
        i += 4;
    }
    out
}

/// Um DTC lido: código legível e byte de status.
#[derive(Debug, Clone)]
pub struct Dtc {
    pub code: String,
    pub status: u8,
}

/// DTCs de um módulo.
#[derive(Debug, Clone)]
pub struct ModuleDtcs {
    pub module: String,
    pub dtcs: Vec<Dtc>,
}

/// Resultado da limpeza de DTCs (UDS 0x14) em um módulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearOutcome {
    /// Resposta positiva `54`: memória de falhas apagada.
    Cleared,
    /// Resposta negativa `7F 14 <NRC>` (ex.: 0x22 condições não atendidas).
    Refused(u8),
    /// Nenhuma resposta do módulo dentro do tempo.
    NoResponse,
}

/// Classifica as respostas de um `14 FF FF FF` para o módulo `resp`.
///
/// Uma positiva (`54`) vence qualquer negativa; o NRC `0x78` (resposta
/// pendente) não é recusa — o módulo ainda vai responder — e é ignorado.
pub fn classify_clear(answers: &[IsoTpResponse], resp: u16) -> ClearOutcome {
    let mine = || answers.iter().filter(|r| r.can_id == resp as u32);
    if mine().any(|r| r.service.first() == Some(&0x54)) {
        return ClearOutcome::Cleared;
    }
    let refused = mine().find_map(|r| match r.service.as_slice() {
        [0x7F, 0x14, nrc, ..] if *nrc != 0x78 => Some(*nrc),
        _ => None,
    });
    match refused {
        Some(nrc) => ClearOutcome::Refused(nrc),
        None => ClearOutcome::NoResponse,
    }
}

/// Sessão de diagnóstico com o veículo. Só lê, exceto [`Self::clear_dtcs`].
pub struct CarSession<T: Transport<Error: 'static>> {
    pub dev: Device<T>,
    pub baud: u32,
}

impl CarSession<SerialTransport> {
    /// Abre a porta serial, inicializa o adaptador e abre o canal ISO-TP.
    pub fn open_serial(port: &str, baud: u32) -> Result<(Self, u32), String> {
        let transport = SerialTransport::open(port, baud)?;
        let mut dev = Device::new(transport);
        dev.timeout = Duration::from_millis(2500);
        let info = dev
            .open()
            .map_err(|e| format!("abrir dispositivo: {e:?}"))?;
        dev.connect_iso_tp(baud)
            .map_err(|e| format!("conectar ISO-TP: {e:?}"))?;
        Ok((Self { dev, baud }, info.status))
    }
}

#[cfg(target_os = "android")]
impl CarSession<UsbFdTransport> {
    /// Abre a partir de um fd do `UsbManager` (Android USB-OTG), inicializa o
    /// adaptador e abre o canal ISO-TP. EXPERIMENTAL — ver `usb_fd.rs`.
    pub fn open_usb_fd(fd: std::os::fd::RawFd, baud: u32) -> Result<(Self, u32), String> {
        let transport = UsbFdTransport::open_fd(fd, baud)?;
        let mut dev = Device::new(transport);
        dev.timeout = Duration::from_millis(2500);
        let info = dev
            .open()
            .map_err(|e| format!("abrir dispositivo: {e:?}"))?;
        dev.connect_iso_tp(baud)
            .map_err(|e| format!("conectar ISO-TP: {e:?}"))?;
        Ok((Self { dev, baud }, info.status))
    }
}

impl<T: Transport<Error: 'static>> CarSession<T> {
    /// Lê o VIN do powertrain (0x22 F190 em 0x7E0).
    pub fn read_vin(&mut self) -> Option<String> {
        let _ = self.dev.add_flow_control_filter(0x7E8, 0x7E0);
        let resp = self
            .dev
            .request_iso_tp(0x7E0, &[0x22, 0xF1, 0x90], Duration::from_millis(900))
            .ok()?;
        for r in &resp {
            if r.service.len() >= 3
                && r.service[0] == 0x62
                && r.service[1] == 0xF1
                && r.service[2] == 0x90
            {
                let vin: String = r.service[3..]
                    .iter()
                    .map(|&b| b as char)
                    .filter(|c| c.is_ascii_graphic())
                    .collect();
                return Some(vin);
            }
        }
        None
    }

    /// Varre quais módulos respondem ao TesterPresent (0x3E 00).
    pub fn scan(&mut self, modules: &[ModuleAddr]) -> Vec<ModuleAddr> {
        for (_, req, resp) in modules {
            let _ = self.dev.add_flow_control_filter(*resp, *req);
        }
        let mut present = Vec::new();
        for (name, req, resp) in modules {
            let answers = self
                .dev
                .request_iso_tp(*req, &[0x3E, 0x00], Duration::from_millis(220))
                .unwrap_or_default();
            let ok = answers.iter().any(|r| {
                r.can_id == *resp as u32
                    && r.service.first().is_some_and(|&b| b == 0x7E || b == 0x7F)
            });
            if ok {
                present.push((name.clone(), *req, *resp));
            }
        }
        present
    }

    /// Lê os DTCs (0x19 02 FF) dos módulos informados.
    pub fn read_dtcs(&mut self, present: &[ModuleAddr]) -> Vec<ModuleDtcs> {
        let mut out = Vec::new();
        for (name, req, resp) in present {
            let answers = self
                .dev
                .request_iso_tp(*req, &[0x19, 0x02, 0xFF], Duration::from_millis(700))
                .unwrap_or_default();
            let Some(r) = answers
                .iter()
                .find(|r| r.can_id == *resp as u32 && r.service.first() == Some(&0x59))
            else {
                continue;
            };
            let dtcs = decode_dtcs(&r.service)
                .into_iter()
                .map(|([b0, b1, b2], status)| Dtc {
                    code: dtc_code(b0, b1, b2),
                    status,
                })
                .collect::<Vec<_>>();
            if !dtcs.is_empty() {
                out.push(ModuleDtcs {
                    module: name.clone(),
                    dtcs,
                });
            }
        }
        out
    }

    /// Apaga os DTCs (UDS 0x14, grupo 0xFFFFFF = todos) dos módulos informados.
    /// ESCREVE no veículo (zera a memória de falhas). Releia os DTCs depois.
    pub fn clear_dtcs(&mut self, modules: &[ModuleAddr]) -> Vec<(String, ClearOutcome)> {
        let mut out = Vec::new();
        for (name, req, resp) in modules {
            let _ = self.dev.add_flow_control_filter(*resp, *req);
            // Coleta por tempo fixo: cobre o `7F 14 78` (pendente) + a `54` final
            // de módulos que demoram a apagar a EEPROM.
            let answers = self
                .dev
                .request_iso_tp(*req, &[0x14, 0xFF, 0xFF, 0xFF], Duration::from_millis(1200))
                .unwrap_or_default();
            out.push((name.clone(), classify_clear(&answers, *resp)));
        }
        out
    }

    /// Lê um DID por UDS 0x22 e devolve os bytes de dado (após `62 <did>`).
    pub fn read_did_raw(&mut self, req: u16, resp: u16, did: u16) -> Option<Vec<u8>> {
        let _ = self.dev.add_flow_control_filter(resp, req);
        let hi = (did >> 8) as u8;
        let lo = (did & 0xff) as u8;
        // 2500 ms acomoda DIDs grandes (ex.: o CCF 0xDE00, 1024 bytes).
        let answers = self
            .dev
            .request_iso_tp(req, &[0x22, hi, lo], Duration::from_millis(2500))
            .ok()?;
        for r in &answers {
            if r.can_id == resp as u32
                && r.service.len() >= 3
                && r.service[0] == 0x62
                && r.service[1] == hi
                && r.service[2] == lo
            {
                return Some(r.service[3..].to_vec());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode() {
        assert_eq!(dtc_code(0x5A, 0x13, 0x08), "C1A13-08");
        assert_eq!(dtc_code(0x06, 0x71, 0x00), "P0671");
        let svc = [0x59, 0x02, 0xFF, 0x5A, 0x13, 0x08, 0x2A];
        assert_eq!(decode_dtcs(&svc), vec![([0x5A, 0x13, 0x08], 0x2A)]);
    }

    fn r(can_id: u32, service: &[u8]) -> IsoTpResponse {
        IsoTpResponse {
            can_id,
            service: service.to_vec(),
        }
    }

    #[test]
    fn clear_positive() {
        let a = [r(0x7E8, &[0x54])];
        assert_eq!(classify_clear(&a, 0x7E8), ClearOutcome::Cleared);
    }

    #[test]
    fn clear_pending_then_positive() {
        let a = [r(0x7E8, &[0x7F, 0x14, 0x78]), r(0x7E8, &[0x54])];
        assert_eq!(classify_clear(&a, 0x7E8), ClearOutcome::Cleared);
    }

    #[test]
    fn clear_pending_only_is_no_response() {
        let a = [r(0x7E8, &[0x7F, 0x14, 0x78])];
        assert_eq!(classify_clear(&a, 0x7E8), ClearOutcome::NoResponse);
    }

    #[test]
    fn clear_refused() {
        let a = [r(0x7E8, &[0x7F, 0x14, 0x22])];
        assert_eq!(classify_clear(&a, 0x7E8), ClearOutcome::Refused(0x22));
    }

    #[test]
    fn clear_ignores_other_modules() {
        let a = [r(0x7E9, &[0x54]), r(0x7EA, &[0x7F, 0x14, 0x31])];
        assert_eq!(classify_clear(&a, 0x7E8), ClearOutcome::NoResponse);
    }
}
