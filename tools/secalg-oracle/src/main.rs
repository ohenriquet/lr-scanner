//! Oráculo offline de chave de SecurityAccess.
//!
//! Carrega a `SecAlg.dll` do próprio SDD (binário 32 bits) e chama a função
//! exportada `SECALG_ComputeSecurityKey` para gerar a chave a partir da
//! semente e da constante do módulo. Assim as chaves são idênticas às que o
//! SDD enviaria, sem reimplementar o algoritmo. Tudo offline: não toca no
//! veículo. Compilar para 32 bits: `--target i686-pc-windows-gnu`.
//!
//! Assinatura reconstruída da desmontagem (cdecl):
//!   int SECALG_ComputeSecurityKey(int algoId, const u8* seed,
//!                                 u8* keyOut, int p4, const u8* constant, int p6)
//! semente = 3 bytes, constante = 5 bytes, keyOut = 3 bytes, retorno 3 = ok.

use anyhow::{Context, Result};
use clap::Parser;

type ComputeFn = unsafe extern "C" fn(
    i32,       // algoId
    *const u8, // seed (usa 3 bytes)
    *mut u8,   // keyOut (3 bytes)
    i32,       // p4 (desconhecido; tentamos 0)
    *const u8, // constant (5 bytes)
    i32,       // p6 (desconhecido; tentamos 0)
) -> i32;

// GetSecurityKey(algo, seed_ptr, seedLen, out_ptr, p5, constant_ptr, p7)
type GetKeyFn = unsafe extern "C" fn(i32, *const u8, i32, *mut u8, i32, *const u8, i32) -> i32;

#[derive(Parser)]
#[command(
    name = "secalg-oracle",
    about = "Gera chaves de SecurityAccess via SecAlg.dll (offline)"
)]
struct Cli {
    /// Caminho da SecAlg.dll do SDD.
    #[arg(
        long,
        default_value = r"C:\Program Files (x86)\JLR\IDS\Runtime\SecAlg.dll"
    )]
    dll: String,
    /// Semente em hex (3 bytes, ex.: 1A2B3C).
    #[arg(long)]
    seed: String,
    /// Constante do módulo em hex (5 bytes).
    #[arg(long)]
    constant: String,
    /// ID do algoritmo. Se omitido, varre os conhecidos (4,10,31,32).
    #[arg(long)]
    algo: Option<i32>,
}

fn parse_hex(s: &str) -> Result<Vec<u8>> {
    let c: String = s.chars().filter(|ch| !ch.is_whitespace()).collect();
    anyhow::ensure!(c.len() % 2 == 0, "hex ímpar");
    (0..c.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&c[i..i + 2], 16).map_err(Into::into))
        .collect()
}

fn hexs(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join("")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let seed = parse_hex(&cli.seed)?;
    let constant = parse_hex(&cli.constant)?;
    anyhow::ensure!(seed.len() >= 3, "semente precisa de ao menos 3 bytes");
    anyhow::ensure!(constant.len() >= 5, "constante precisa de 5 bytes");

    // Semente passada como 4 bytes (a DLL copia 4 e usa 3).
    let mut seed4 = [0u8; 4];
    seed4[..3].copy_from_slice(&seed[..3]);

    let lib = unsafe { libloading::Library::new(&cli.dll) }
        .with_context(|| format!("carregar {}", cli.dll))?;
    let compute: libloading::Symbol<ComputeFn> = unsafe { lib.get(b"SECALG_ComputeSecurityKey\0") }
        .context("símbolo SECALG_ComputeSecurityKey")?;

    let algos: Vec<i32> = match cli.algo {
        Some(a) => vec![a],
        None => vec![4, 10, 31, 32],
    };

    // Chave da nossa implementação Rust (porte do GetSecurityCodeISO14229).
    let mut s3 = [0u8; 3];
    s3.copy_from_slice(&seed[..3]);
    let mut c5 = [0u8; 5];
    c5.copy_from_slice(&constant[..5]);
    let rust_key = jlr_security::iso14229_key(s3, c5);

    println!(
        "semente={} constante={}",
        hexs(&seed[..3]),
        hexs(&constant[..5])
    );
    println!("Rust iso14229: chave={}", hexs(&rust_key));
    for algo in algos {
        let mut key = [0u8; 3];
        let rc = unsafe {
            compute(
                algo,
                seed4.as_ptr(),
                key.as_mut_ptr(),
                0,
                constant.as_ptr(),
                0,
            )
        };
        let flag = if key == rust_key && rc != 0 {
            "  <== BATE com o Rust"
        } else {
            ""
        };
        println!(
            "SecAlg Compute algo {algo:>3}: rc={rc}  chave={}{flag}",
            hexs(&key)
        );
    }

    // Tenta também GetSecurityKey (7 params, com comprimento explícito).
    if let Ok(getkey) = unsafe { lib.get::<GetKeyFn>(b"SECALG_GetSecurityKey\0") } {
        for algo in [4, 10, 31, 32] {
            for seedlen in [3i32, 4] {
                let mut key = [0u8; 3];
                let rc = unsafe {
                    getkey(
                        algo,
                        seed4.as_ptr(),
                        seedlen,
                        key.as_mut_ptr(),
                        0,
                        constant.as_ptr(),
                        constant.len() as i32,
                    )
                };
                let flag = if key == rust_key && rc != 0 {
                    "  <== BATE com o Rust"
                } else {
                    ""
                };
                if rc != 0 || key != [0, 0, 0] {
                    println!(
                        "SecAlg GetKey algo {algo:>3} seedlen {seedlen}: rc={rc}  chave={}{flag}",
                        hexs(&key)
                    );
                }
            }
        }
    }
    Ok(())
}
