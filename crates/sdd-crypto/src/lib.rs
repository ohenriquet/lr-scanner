//! Codec dos arquivos `.exml` do JLR SDD.
//!
//! Os arquivos `.exml` da instalação do SDD são XML (UTF-8) cifrados com
//! 3DES (Triple DES, EDE) em modo ECB, com preenchimento PKCS#5/#7 e sem IV.
//! A chave de 24 bytes é um artefato público de engenharia reversa (ver
//! `smartgauges/exml` e o porte Python `5555l/sdd_exml`); ela decifra o
//! formato de arquivo do SDD e NÃO tem relação com o SecurityAccess (0x27)
//! dos módulos do veículo.
//!
//! O decodificador foi validado contra o par plano/cifrado existente na
//! instalação (`IDS\Xml\JLR_VSV_CONFIG.xml` ↔ `.exml`) e contra os 552
//! arquivos `.exml` da raiz e das pastas L319.
//!
//! # Exemplo
//! ```no_run
//! let cifrado = std::fs::read("CCF_DATA_L319_201000.exml").unwrap();
//! let xml = sdd_crypto::decrypt(&cifrado).unwrap();
//! assert!(xml.starts_with(b"<?xml"));
//! ```

use cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use des::TdesEde3;

/// Chave 3DES (24 bytes) do codec `.exml` do SDD.
///
/// DES ignora o bit de paridade (LSB) de cada byte da chave, então estes
/// bytes ASCII são usados diretamente, sem ajuste de paridade.
const EXML_KEY: [u8; 24] = *b"YmZwZlQrQ3V4dVltNTArWE9s";

const BLOCK: usize = 8;

/// Erros do codec `.exml`.
#[derive(Debug, thiserror::Error)]
pub enum ExmlError {
    /// O tamanho do texto cifrado não é múltiplo do bloco de 8 bytes.
    #[error("texto cifrado com {0} bytes não é múltiplo de 8")]
    NotBlockAligned(usize),
    /// Entrada vazia (3DES-ECB com PKCS#5 nunca produz 0 bytes).
    #[error("entrada vazia")]
    Empty,
    /// O preenchimento PKCS#5/#7 ao final do texto decifrado é inválido.
    #[error("preenchimento PKCS#5 inválido (byte final = {0})")]
    BadPadding(u8),
}

fn cipher() -> TdesEde3 {
    // A chave tem tamanho fixo e válido; `new_from_slice` não falha.
    TdesEde3::new_from_slice(&EXML_KEY).expect("chave 3DES de 24 bytes é válida")
}

/// Decifra o conteúdo de um arquivo `.exml`, removendo o preenchimento PKCS#5,
/// e devolve o XML em texto (bytes UTF-8).
pub fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, ExmlError> {
    if ciphertext.is_empty() {
        return Err(ExmlError::Empty);
    }
    if ciphertext.len() % BLOCK != 0 {
        return Err(ExmlError::NotBlockAligned(ciphertext.len()));
    }
    let c = cipher();
    let mut out = ciphertext.to_vec();
    for chunk in out.chunks_exact_mut(BLOCK) {
        c.decrypt_block(chunk.into());
    }
    strip_pkcs5(&mut out)?;
    Ok(out)
}

/// Cifra um XML em texto para o formato `.exml` (aplica PKCS#5 e 3DES-ECB).
///
/// Operação inversa de [`decrypt`], usada em testes de ida-e-volta e,
/// futuramente, para gravar arquivos de configuração alterados.
pub fn encrypt(plaintext: &[u8]) -> Vec<u8> {
    let mut out = plaintext.to_vec();
    pad_pkcs5(&mut out);
    let c = cipher();
    for chunk in out.chunks_exact_mut(BLOCK) {
        c.encrypt_block(chunk.into());
    }
    out
}

/// Remove o preenchimento PKCS#5/#7 no fim do buffer, validando-o.
fn strip_pkcs5(buf: &mut Vec<u8>) -> Result<(), ExmlError> {
    let pad = *buf.last().ok_or(ExmlError::Empty)? as usize;
    if pad == 0 || pad > BLOCK || pad > buf.len() {
        return Err(ExmlError::BadPadding(pad as u8));
    }
    let start = buf.len() - pad;
    if buf[start..].iter().any(|&b| b as usize != pad) {
        return Err(ExmlError::BadPadding(pad as u8));
    }
    buf.truncate(start);
    Ok(())
}

/// Acrescenta preenchimento PKCS#5/#7 para alinhar ao bloco de 8 bytes.
///
/// Quando o comprimento já é múltiplo de 8, acrescenta um bloco inteiro de
/// `0x08`, como exige o PKCS#5.
fn pad_pkcs5(buf: &mut Vec<u8>) {
    let pad = BLOCK - (buf.len() % BLOCK);
    buf.extend(std::iter::repeat(pad as u8).take(pad));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserva_conteudo() {
        let xml = b"<?xml version=\"1.0\"?><root><a>valor</a></root>".to_vec();
        let enc = encrypt(&xml);
        assert_eq!(enc.len() % BLOCK, 0);
        let dec = decrypt(&enc).unwrap();
        assert_eq!(dec, xml);
    }

    #[test]
    fn roundtrip_quando_ja_alinhado_acrescenta_bloco_inteiro() {
        // 8 bytes: o PKCS#5 exige um bloco de preenchimento extra.
        let xml = b"12345678".to_vec();
        let enc = encrypt(&xml);
        assert_eq!(enc.len(), 16);
        assert_eq!(decrypt(&enc).unwrap(), xml);
    }

    #[test]
    fn rejeita_tamanho_desalinhado() {
        assert!(matches!(
            decrypt(&[0u8; 7]),
            Err(ExmlError::NotBlockAligned(7))
        ));
    }

    #[test]
    fn rejeita_entrada_vazia() {
        assert!(matches!(decrypt(&[]), Err(ExmlError::Empty)));
    }
}
