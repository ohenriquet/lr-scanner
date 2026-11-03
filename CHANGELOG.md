# Changelog

## v0.1.0-beta — primeira beta multiplataforma

Primeira beta pública. As quatro plataformas compilam; leitura e diagnóstico
foram validados no carro real (Land Rover Discovery 4 / L319 MY12, 3.0 SDV6).

### Funciona
- **Núcleo Rust**: decifra os `.exml` do SDD (3DES), parseia módulos/DIDs/DTCs/
  CCF e gera pacotes por veículo (`sdd-import`); protocolo nativo do Mongoose
  Pro JLR; UDS/ISO-TP (incl. ISO-TP de software para passar do buffer de 128 B
  do BCM).
- **Diagnóstico no carro**: VIN, varredura de módulos, DTCs com o help do SDD
  por módulo, live data com escala/unidade, e leitura+decodificação do CCF.
- **App Flutter** nas quatro plataformas, com UI compartilhada:
  - **Windows (nativo)**: FFI direto com o Mongoose (porta COM). Rápido, sem
    servidor/navegador.
  - **macOS**: alvo pronto para compilar num Mac (FFI).
  - **Android**: APK com o núcleo Rust (`.so`); modo referência completo.
  - **Web**: via `car-scan serve` (ponte HTTP local) + navegador.
- **Tema claro/escuro** com alternador no app (segue o sistema por padrão).
- Conexão rápida (mostra VIN/módulos na hora; falhas lidas por módulo em 2º
  plano) e as falhas separadas em **confirmadas** vs histórico.
- Ferramenta `car-scan` (CLI): scan, DTCs, live, leitura de CCF, e `serve`.

### Experimental / limitações
- **Android — conexão com o carro (USB-OTG)**: implementada ponta a ponta
  (fd do `UsbManager` → `nusb` no Rust), porém **ainda não validada em
  dispositivo**. Ver `app/jlr_app/ANDROID.md`.
- **Gravação do CCF**: bloqueada por um **Secondary Boot Loader (SBL)**
  proprietário da JLR, que não vem no SDD local (o SDD o baixa online). Toda a
  leitura/decodificação do CCF funciona. Ver `docs/security-ccf-write.md`
  (ajuda procurada).
- **macOS**: compila/testa só num Mac (não a partir do Windows).

### Notas
- O app **não embute dados da JLR**; o usuário gera o pacote do próprio SDD.
- Código próprio sob MIT; crate `mongoose` sob LGPL-2.1 (derivado de RE pública).
