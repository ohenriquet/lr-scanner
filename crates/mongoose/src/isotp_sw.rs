//! ISO-TP (ISO 15765-2) de transmissão e recepção **em software**, sobre um
//! canal CAN cru do adaptador.
//!
//! O firmware do Mongoose limita a transmissão ISO-TP a 128 bytes de payload
//! (buffer de TX). Para gravar o CCF (~1 KB) é preciso fatiar a mensagem em
//! quadros CAN de 8 bytes e tratar o controle de fluxo do módulo (BS/STmin,
//! múltiplos blocos) aqui no host. Endereçamento normal de 11 bits, sem
//! endereçamento estendido.

use std::time::{Duration, Instant};

use crate::client::{ClientError, Device};
use crate::Transport;

/// Byte de preenchimento dos quadros curtos (pad para 8 bytes).
const PAD: u8 = 0x00;

/// Erros do ISO-TP de software.
#[derive(Debug, thiserror::Error)]
pub enum IsoTpError {
    #[error("tempo esgotado esperando {0}")]
    Timeout(&'static str),
    #[error("controle de fluxo recusado (0x32 overflow)")]
    Overflow,
    #[error("sequência de quadro consecutiva inesperada: {got:#x} esperado {want:#x}")]
    BadSequence { got: u8, want: u8 },
    #[error("quadro malformado")]
    Malformed,
}

/// Faz uma transação ISO-TP: envia `data` para `req_id` e devolve a resposta
/// reassemblada que vier de `resp_id`.
pub fn request<T: Transport>(
    dev: &mut Device<T>,
    req_id: u16,
    resp_id: u16,
    data: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, Box<dyn std::error::Error>>
where
    T::Error: std::fmt::Debug + 'static,
{
    send(dev, req_id, resp_id, data)?;
    recv(dev, req_id, resp_id, timeout)
}

/// Envia uma mensagem ISO-TP (segmentando se preciso), tratando o controle de
/// fluxo recebido de `resp_id`.
pub fn send<T: Transport>(
    dev: &mut Device<T>,
    req_id: u16,
    resp_id: u16,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error>>
where
    T::Error: std::fmt::Debug + 'static,
{
    if data.len() <= 7 {
        // Single Frame.
        let mut f = Vec::with_capacity(8);
        f.push(data.len() as u8); // 0x0L
        f.extend_from_slice(data);
        pad(&mut f);
        dev.can_send(req_id, &f).map_err(box_err)?;
        return Ok(());
    }

    // First Frame: 0x1X XX + 6 bytes.
    let len = data.len();
    let mut ff = Vec::with_capacity(8);
    ff.push(0x10 | ((len >> 8) & 0x0f) as u8);
    ff.push((len & 0xff) as u8);
    ff.extend_from_slice(&data[..6]);
    dev.can_send(req_id, &ff).map_err(box_err)?;

    // Espera o Flow Control do módulo.
    let (mut bs, mut stmin) = wait_fc(dev, resp_id, Duration::from_millis(1000))?;

    let mut idx = 6usize;
    let mut sn: u8 = 1;
    let mut in_block = 0u32;
    while idx < len {
        let end = (idx + 7).min(len);
        let mut cf = Vec::with_capacity(8);
        cf.push(0x20 | (sn & 0x0f));
        cf.extend_from_slice(&data[idx..end]);
        pad(&mut cf);
        dev.can_send(req_id, &cf).map_err(box_err)?;
        idx = end;
        sn = sn.wrapping_add(1) & 0x0f;
        if stmin > 0 {
            std::thread::sleep(Duration::from_millis(stmin as u64));
        }
        // Respeita o BlockSize: após BS quadros, espera novo FC (BS=0 = sem limite).
        if bs != 0 {
            in_block += 1;
            if in_block >= bs as u32 && idx < len {
                let fc = wait_fc(dev, resp_id, Duration::from_millis(1000))?;
                bs = fc.0;
                stmin = fc.1;
                in_block = 0;
            }
        }
    }
    Ok(())
}

/// Espera um quadro de controle de fluxo de `resp_id`. Devolve (BlockSize, STmin ms).
fn wait_fc<T: Transport>(
    dev: &mut Device<T>,
    resp_id: u16,
    timeout: Duration,
) -> Result<(u8, u8), Box<dyn std::error::Error>>
where
    T::Error: std::fmt::Debug + 'static,
{
    let deadline = Instant::now() + timeout;
    loop {
        let frames = dev.can_poll(Duration::from_millis(60)).map_err(box_err)?;
        for (id, d) in frames {
            if id != resp_id as u32 || d.is_empty() {
                continue;
            }
            match d[0] & 0xf0 {
                0x30 => {
                    let flag = d[0] & 0x0f;
                    if flag == 2 {
                        return Err(Box::new(IsoTpError::Overflow));
                    }
                    if flag == 1 {
                        // WAIT: continua esperando outro FC.
                        continue;
                    }
                    let bs = d.get(1).copied().unwrap_or(0);
                    let stmin = decode_stmin(d.get(2).copied().unwrap_or(0));
                    return Ok((bs, stmin));
                }
                _ => continue,
            }
        }
        if Instant::now() >= deadline {
            return Err(Box::new(IsoTpError::Timeout("flow control")));
        }
    }
}

/// Recebe e reassembla uma mensagem ISO-TP de `resp_id`, enviando o nosso
/// Flow Control (CTS, BS=0) quando o módulo manda um First Frame.
pub fn recv<T: Transport>(
    dev: &mut Device<T>,
    req_id: u16,
    resp_id: u16,
    timeout: Duration,
) -> Result<Vec<u8>, Box<dyn std::error::Error>>
where
    T::Error: std::fmt::Debug + 'static,
{
    let deadline = Instant::now() + timeout;
    let mut msg: Vec<u8> = Vec::new();
    let mut expected = 0usize;
    let mut sn: u8 = 1;
    let mut assembling = false;

    loop {
        let frames = dev.can_poll(Duration::from_millis(80)).map_err(box_err)?;
        for (id, d) in frames {
            if id != resp_id as u32 || d.is_empty() {
                continue;
            }
            match d[0] & 0xf0 {
                0x00 => {
                    // Single Frame.
                    let l = (d[0] & 0x0f) as usize;
                    if d.len() < 1 + l {
                        return Err(Box::new(IsoTpError::Malformed));
                    }
                    return Ok(d[1..1 + l].to_vec());
                }
                0x10 => {
                    // First Frame.
                    expected =
                        (((d[0] & 0x0f) as usize) << 8) | d.get(1).copied().unwrap_or(0) as usize;
                    msg.clear();
                    msg.extend_from_slice(&d[2..d.len().min(8)]);
                    assembling = true;
                    sn = 1;
                    // Envia Flow Control CTS (BS=0, STmin=0).
                    dev.can_send(req_id, &[0x30, 0x00, 0x00, PAD, PAD, PAD, PAD, PAD])
                        .map_err(box_err)?;
                }
                0x20 => {
                    if !assembling {
                        continue;
                    }
                    let got = d[0] & 0x0f;
                    if got != sn {
                        return Err(Box::new(IsoTpError::BadSequence { got, want: sn }));
                    }
                    sn = sn.wrapping_add(1) & 0x0f;
                    let remaining = expected.saturating_sub(msg.len());
                    let take = remaining.min(d.len() - 1);
                    msg.extend_from_slice(&d[1..1 + take]);
                    if msg.len() >= expected {
                        return Ok(msg);
                    }
                }
                _ => continue,
            }
        }
        if Instant::now() >= deadline {
            return Err(Box::new(IsoTpError::Timeout("resposta")));
        }
    }
}

fn pad(f: &mut Vec<u8>) {
    while f.len() < 8 {
        f.push(PAD);
    }
}

/// Decodifica o STmin do FC em milissegundos (0x00-0x7F = ms; 0xF1-0xF9 = sub-ms).
fn decode_stmin(raw: u8) -> u8 {
    if raw <= 0x7f {
        raw
    } else {
        1 // sub-ms: dorme 1 ms por segurança
    }
}

fn box_err<E: std::fmt::Debug + 'static>(e: ClientError<E>) -> Box<dyn std::error::Error> {
    Box::new(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stmin_ms() {
        assert_eq!(decode_stmin(0x00), 0);
        assert_eq!(decode_stmin(0x0a), 10);
        assert_eq!(decode_stmin(0x7f), 127);
        assert_eq!(decode_stmin(0xf1), 1);
    }
}
