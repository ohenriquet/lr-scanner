# SecurityAccess e gravação do CCF (L319 BCM) — notas

Objetivo: habilitar opções de conforto (welcome/approach/puddle lights etc.)
gravando o CCF no BCM. Isso exige desbloqueio por SecurityAccess (UDS 0x27).
Fonte: dados do SDD decifrados + desmontagem da `SecAlg.dll` + cruzamento com
engenharia reversa pública do EUCD Ford/JLR.

## O que já se sabe (do SDD decifrado)

- O CCF é lido/gravado no BCM por DID 0xDE00 (serviço 0x22 ler, 0x2E gravar),
  sessão estendida (0x10 0x03). Confirmado: a leitura de 0xDE00 devolve 1024
  bytes (validado no carro).
- Gravar DIDs de configuração do BCM exige `security_level_1` (MDX_BCM).
- `BCM.xml` (swdl) deste BCM: `SECURITY_ALGORITHM_ID=0`,
  `NUMBER_OF_SECURITY_CONSTANTS=5`, `KEY_CONST_REF=168`.
- `Security.xml` grupo Index 168 (Module=BCM, 5 SecData) é o deste BCM:
  - SecLevel 0x01 (P, programação), constante DC0163
  - **SecLevel 0x03 (D, diagnóstico), constante DC0164** ← grava CCF
  - SecLevel 0x11 (D), DC0165
  - SecLevel 0x21 (D), DC0165
  - SecLevel 0x99 (X, especial), DC0009
- Para o CCF (sessão diag + level 1): pedir semente com `0x27 0x03`, enviar
  chave com `0x27 0x04`, usando a constante **DC0164** (5 bytes = 40 bits) e o
  algoritmo ID 0.

## Cruzamento com a comunidade

Módulos EUCD legados Ford/JLR usam um LFSR de 24 bits: semente de 3 bytes,
chave de 3 bytes, com 40 bits de segredo. Os 40 bits = exatamente as 5
constantes por módulo do SDD. Diferença importante: o SDD **já traz o segredo**
(DC0164), então não é preciso quebrar nada — só aplicar o LFSR. A forma exata
(taps, ordem dos bytes, iterações) vem da `SecAlg.dll`.

Referências públicas (metodologia, não a chave da JLR):
- Ford F-150 IPC SecurityAccess (LFSR 24 bits, 40 bits de segredo):
  https://github.com/Ol-Swamp-Donkey/ford-f150-ipc-securityaccess
- Mercedes IC204 (método de RE de seed/key com Ghidra):
  https://github.com/Mattwmaster58/ic204

## Sequência de gravação planejada (NÃO executada ainda)

1. `0x10 0x03` no BCM (0x726) — sessão estendida. Reversível.
2. `0x27 0x03` → BCM responde `0x67 0x03 <semente>` (pedir semente é seguro,
   não conta como tentativa falha).
3. chave = algoritmo0(semente, DC0164).
4. `0x27 0x04 <chave>` → `0x67 0x04` (ok) ou `0x7F 27 <NRC>`.
   ATENÇÃO: chave errada repetida ativa atraso anti-brute-force no módulo.
5. Montar o CCF alterado: ler os 1024 bytes atuais, mudar só o(s) byte(s) do
   parâmetro desejado, recalcular os checksums do CCF (CHECKSUM_CCF_EUCD) e a
   data (DATECALC_JAG_X150).
6. Gravar `0x2E 0xDE00 <1024 bytes>` (ou a sequência VBF 0x34/0x36/0x37 se o
   SDD usar essa via para o CARCFG).
7. Reler 0xDE00 e conferir byte a byte. Tester present durante o processo.

## Pendências antes de qualquer gravação

- [x] Algoritmo extraído: `GetSecurityCodeISO14229` (LFSR init 0xC541A9, poly
  0x109028, 64 iterações). Implementado no crate `jlr-security`.
- Reconstruir o checksum do CCF (CHECKSUM_CCF_EUCD) — provável nas DLLs de CCF.
- [x] Gerador VALIDADO contra a `SecAlg.dll` do próprio SDD (export
  `SECALG_GetSecurityKey`, algoritmo 32, semente de 3 bytes): 12/12 sementes
  produzem chave idêntica. Oráculo 32 bits em `tools/secalg-oracle`.
- Fazer sempre backup do CCF antes (já temos um) e ter plano de restauração.

## Segurança do procedimento

Pedir semente (0x27 03) é leitura e pode ser repetido. Só o envio de chave
errada (0x27 04) penaliza. A estratégia é não enviar chave até ter alta
confiança (algoritmo conferido e, de preferência, validado contra captura).

## Checksum (reconstrução em andamento)

Algoritmo confirmado da `VbfConversion.dll` (`PerformEUCD_CCCChecksum`): soma de
8 bits dos bytes de uma faixa, `checksum = soma & 0xff`. Pistas no CCF real do
carro: `(-soma8(bloco1)) & 0xff == 0x53`, que é o `VEH_BUILD_CHECKSUM_BYTE_1`
armazenado — ou seja, checksum de complemento de dois (soma de dados + byte de
checksum ≡ 0 mod 256). Posições na imagem do DID 0xDE00: EUCD checksum em
562/563, VEH_BUILD em 696/697 (bloco 5, bytes 2/3 e 136/137). Falta fixar a
faixa exata de cada um (o SDD soma sobre o buffer VBF, cujo layout difere da
imagem crua). Enquanto não fechado, `ccf-set` fica bloqueado; `ccf-writeback`
(regravar idêntico) não precisa do checksum.

## Sequência de gravação mapeada no carro (MY12 BCM)

Validado no BCM real da Discovery 4, via ISO-TP de software:
1. `0x10 03` (sessão estendida) — ok.
2. `0x27 03`/`0x04` desbloqueio DIAGNÓSTICO (constante DC0164) — ACEITO.
3. `0x10 02` (sessão de programação) — ok (módulo segue vivo, 0x3E → 0x7E).
4. `0x27 01`/`0x02` desbloqueio de PROGRAMAÇÃO (constante DC0163) — ACEITO.
   O mesmo algoritmo `jlr-security` vale para o nível de programação.
5. `0x34` RequestDownload — suportado SÓ na sessão de programação; com a
   segurança ok responde `0x7F 34 31` (requestOutOfRange) para os endereços
   do CCF (0xFFAC00, blocos e região). Ou seja, o 0x34 padrão aponta para a
   região de firmware, não para o CCF.

Conclusão: a gravação do CCF NÃO é um RequestDownload comum para 0xFFAC00.
É um mecanismo específico do SDD (provável DID/rotina proprietária ou um
RequestDownload com endereço/formato lógico diferente). Para fechar com
segurança, o caminho confiável é CAPTURAR o SDD gravando um CCF (shim J2534)
e copiar a sequência exata (formato, endereço, tamanho do bloco, checksum).

Já está validado e pronto: todo o encadeamento de sessão e segurança
(diagnóstico E programação) no módulo real, e o ISO-TP de software para
enviar/receber qualquer tamanho dentro do buffer de 128 bytes do BCM.
