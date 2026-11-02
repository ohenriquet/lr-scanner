# jlr-open-diag

Ferramenta aberta e multiplataforma (Windows 11, macOS, Android) de diagnóstico
e configuração para veículos Jaguar Land Rover, reimplementando as funções do
**JLR SDD** (Symptom Driven Diagnostics) sobre o adaptador **Mongoose Pro JLR**
(J2534, USB).

Primeiro alvo validado: **Land Rover Discovery 4 (L319) 3.0 SDV6 diesel**.
A arquitetura é genérica para toda a cobertura do SDD; outros veículos entram à
medida que são validados.

## Estado atual

Em desenvolvimento inicial. Ver o plano em
`../../.claude/plans/` e o progresso por fase abaixo.

| Fase | Entrega | Estado |
|---|---|---|
| 0 | Ambiente + repositório | feito |
| 1 | `sdd-crypto` (codec `.exml`) | feito |
| 1 | `sdd-data` + `sdd-import` (parsers → SQLite) | feito |
| 2 | `j2534` + shim gravador (Windows) | pendente |
| 3 | `mongoose` (protocolo USB nativo) | pendente |
| 4 | `diag` (UDS, leitura no carro) | pendente |
| 5 | `ccf` (edição de configuração) | pendente |
| 6 | `procedures` (rotinas de serviço) | pendente |
| 7 | app Flutter | pendente |
| 8 | expansão a todos os veículos | pendente |

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
