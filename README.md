# jlr-open-diag

Ferramenta aberta e multiplataforma (Windows 11, macOS, Android) de diagnóstico
e configuração para veículos Jaguar Land Rover, reimplementando as funções do
**JLR SDD** (Symptom Driven Diagnostics) sobre o adaptador **Mongoose Pro JLR**
(J2534, USB).

Primeiro alvo validado: **Land Rover Discovery 4 (L319) 3.0 SDV6 diesel**.
A arquitetura é genérica para toda a cobertura do SDD; outros veículos entram à
medida que são validados.

## Estado atual

Leitura e diagnóstico **funcionam no carro real** (validado num L319 MY12). A
**gravação do CCF** está bloqueada por um binário proprietário da JLR (o SBL) —
ver "Bloqueio conhecido" abaixo.

| Área | Entrega | Estado |
|---|---|---|
| `sdd-crypto` | codec `.exml` (3DES) | ✅ |
| `sdd-data` / `sdd-import` | parsers → SQLite, `decrypt` de `.exml` | ✅ |
| `mongoose` | protocolo USB/serial nativo do adaptador | ✅ |
| `diag` | UDS/ISO-TP: VIN, scan de módulos, DTCs c/ help, live data | ✅ no carro |
| `ccf` (leitura) | ler + decodificar o CCF do BCM | ✅ no carro |
| `ccf` (gravação) | editar e gravar o CCF | ⛔ bloqueado (SBL) |
| app Flutter | UI multiplataforma + ponte FFI/HTTP | 🚧 web ok; Windows nativo / Android / macOS em andamento |
| `j2534-shim` | capturar o SDD gravando (destrava o CCF) | 🚧 planejado |
| `procedures` | rotinas de serviço (DPF, injetores, suspensão…) | ⬜ |
| expansão | todos os veículos cobertos pelo SDD | ⬜ |

## Bloqueio conhecido: gravação do CCF (ajuda procurada)

Ler e decodificar o CCF (configuração do veículo) do BCM funciona. **Gravar**
não: o BCM exige que um **Secondary Boot Loader (SBL)** seja baixado para a sua
RAM antes de aceitar o erase/flash da região de config — sem ele, o erase
responde `7F 31 22` (conditionsNotCorrect), mesmo com sessão de programação e
segurança corretas. Esse SBL **não vem no SDD local**: o SDD o busca no servidor
online da JLR (TOPIx) na hora da gravação. A investigação completa (RE das DLLs,
testes no carro, dados decifrados) está em
[`docs/security-ccf-write.md`](docs/security-ccf-write.md).

**Help wanted (EN):** writing the CCF on an L319 (EUCD) BCM needs JLR's Secondary
Boot Loader (SBL), delivered from JLR online — it is not in a local SDD install.
Everything else (reading/decoding the CCF, DTCs, live data, security access)
works. If you have the **L319 BCM SBL VBF**, or a **J2534 capture of SDD writing
a CCF**, please open an issue — that single piece is what's needed to finish
offline CCF writing.

## Princípio sobre dados e direitos

Este repositório **não contém** dados da Jaguar Land Rover. Os bancos de
diagnóstico (definições de módulo, DTCs, DIDs, mapa do CCF, textos) pertencem à
JLR e ficam na instalação do SDD do próprio usuário. A ferramenta `sdd-import`
lê essa instalação **localmente**, na máquina do dono do veículo, e gera pacotes
de dados que também não são redistribuídos (ver `.gitignore`).

O código próprio é licenciado sob MIT. O crate `mongoose`, por derivar de
engenharia reversa publicada sob LGPL-2.1
(`github.com/dgcarr/Mongoose-USB-2-OBDII-Linux-Driver`), é licenciado sob
LGPL-2.1. Código sob GPL (por exemplo `5555l/ccf_parser`) é usado apenas como
**referência de comportamento**, nunca copiado.

## Aviso

Mexer em módulos de um veículo tem risco. Toda gravação faz backup antes,
exige ignição ligada com o motor desligado e confirmação explícita. Use por sua
conta e risco.

## Ambiente de desenvolvimento

As toolchains ficam em `C:\dev` (portáteis, sem necessidade de administrador):
Rust (toolchain GNU + mingw-w64), Flutter, JDK 21, Ghidra. Carregue o ambiente
com `source env.sh` antes de `cargo`/`flutter`.

```bash
source env.sh
cargo test            # testa todos os crates
```
