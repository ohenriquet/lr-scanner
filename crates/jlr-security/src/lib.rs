//! SecurityAccess (semente/chave) dos módulos EUCD da JLR (ISO 14229).
//!
//! Porte fiel da função `CSecurityAccess::GetSecurityCodeISO14229` da
//! `ModuleSecurityAccess.dll` do SDD (recuperada por desmontagem, mantida
//! fora do repositório). É um LFSR de 24 bits, 64 iterações, com:
//! init `0xC541A9`, polinômio `0x109028`, bit de realimentação `0x800000`.
//! Os mesmos valores aparecem na `SecAlg.dll` (dupla corroboração interna).
//!
//! A entrada é a semente de 3 bytes (do módulo, via `0x27 0x03`) e a
//! constante de 5 bytes do módulo (do `Security.exml` do SDD; é o segredo que
//! o SDD já traz). A saída é a chave de 3 bytes (enviada com `0x27 0x04`).
//!
//! Este algoritmo NÃO embute nenhuma constante da JLR: a constante de 5 bytes
//! é fornecida pelo chamador, lida da instalação do próprio usuário.

/// Valor inicial do LFSR (constante do algoritmo, não um segredo de módulo).
const LFSR_INIT: u32 = 0x00C5_41A9;
/// Bit de realimentação OR-ado antes do XOR do polinômio.
const LFSR_FEEDBACK: u32 = 0x0080_0000;
/// Polinômio XOR-ado quando o bit de saída é 1.
const LFSR_POLY: u32 = 0x0010_9028;
/// Número de iterações.
const ROUNDS: usize = 0x40;

/// Calcula a chave de 3 bytes a partir da semente de 3 bytes e da constante
/// de 5 bytes do módulo.
///
/// `seed` são os 3 bytes da semente na ordem em que o módulo os envia
/// (byte mais significativo primeiro). `constant` são os 5 bytes do módulo.
pub fn iso14229_key(seed: [u8; 3], constant: [u8; 5]) -> [u8; 3] {
    // param_1 = semente como inteiro de 24 bits (big-endian): S2 S1 S0.
    let p = (seed[0] as u32) << 16 | (seed[1] as u32) << 8 | seed[2] as u32;

    // uVar5 inicial = constant[0]<<24 | (p&0xff)<<16 | (p&0xff00) | (p>>16 & 0xff)
    // (embaralha os bytes da semente e antepõe constant[0]).
    let mut uvar5: u32 =
        (constant[0] as u32) << 24 | (p & 0xff) << 16 | (p & 0xff00) | (p >> 16) & 0xff;

    // uVar4 = constant[1..5] como u32 little-endian.
    let mut uvar4: u32 = constant[1] as u32
        | (constant[2] as u32) << 8
        | (constant[3] as u32) << 16
        | (constant[4] as u32) << 24;

    let mut uvar3: u32 = LFSR_INIT;

    for _ in 0..ROUNDS {
        let out_bit = uvar3 ^ uvar5;
        uvar3 >>= 1;
        if out_bit & 1 != 0 {
            uvar3 = (uvar3 | LFSR_FEEDBACK) ^ LFSR_POLY;
        }
        let feed = uvar4 & 1;
        uvar4 >>= 1;
        uvar5 >>= 1;
        if feed != 0 {
            uvar5 |= 0x8000_0000;
        }
    }

    // Reordenação final dos bits de uVar3 (24 bits) na chave de 3 bytes.
    // Transcrição literal da DLL:
    //   key = ((uVar3>>8 & 0xf0 | uVar3>>0x14) + (uVar3>>4 & 0xff)*0x100)*0x100
    //         + (uVar3>>0x10 & 0xf | (byte)(uVar3<<4))
    let hi = ((uvar3 >> 8) & 0xf0) | (uvar3 >> 0x14);
    let mid = (uvar3 >> 4) & 0xff;
    let lo = ((uvar3 >> 0x10) & 0xf) | ((uvar3 << 4) & 0xff);
    let key = hi
        .wrapping_add(mid.wrapping_mul(0x100))
        .wrapping_mul(0x100)
        .wrapping_add(lo)
        & 0xff_ffff;

    [(key >> 16) as u8, (key >> 8) as u8, key as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministico() {
        // Semente e constante arbitrárias: a chave é estável.
        let k1 = iso14229_key([0x12, 0x34, 0x56], [0x11, 0x22, 0x33, 0x44, 0x55]);
        let k2 = iso14229_key([0x12, 0x34, 0x56], [0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(k1, k2);
    }

    #[test]
    fn sementes_diferentes_chaves_diferentes() {
        let c = [0x11, 0x22, 0x33, 0x44, 0x55];
        let a = iso14229_key([0x00, 0x00, 0x01], c);
        let b = iso14229_key([0x00, 0x00, 0x02], c);
        assert_ne!(a, b);
    }

    /// Vetores de regressão gerados pela própria `SecAlg.dll` do SDD (via o
    /// oráculo `secalg-oracle`, algoritmo 32), com uma constante FICTÍCIA
    /// `11 22 33 44 55` (não é segredo de módulo da JLR). A implementação foi
    /// confirmada idêntica à DLL em 12/12 sementes para a constante real do
    /// BCM; estes vetores travam a implementação contra regressões.
    #[test]
    fn vetores_da_dll_constante_ficticia() {
        let c = [0x11, 0x22, 0x33, 0x44, 0x55];
        let casos: &[([u8; 3], [u8; 3])] = &[
            ([0x00, 0x00, 0x00], [0x62, 0x4A, 0x06]),
            ([0x12, 0x34, 0x56], [0xA5, 0x4E, 0x7B]),
            ([0xAB, 0xCD, 0xEF], [0x2D, 0x4A, 0x51]),
            ([0xFF, 0xFF, 0xFF], [0x09, 0xB7, 0xDF]),
            ([0x7F, 0x12, 0x34], [0xA9, 0x8B, 0xA8]),
        ];
        for (seed, esperado) in casos {
            assert_eq!(&iso14229_key(*seed, c), esperado, "semente {seed:02X?}");
        }
    }
}
