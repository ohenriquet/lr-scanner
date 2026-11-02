# Shim gravador J2534 (captura do SDD)

Captura a sequência exata com que o SDD grava o CCF, para reproduzi-la no
`car-scan`. O shim (`monpj_shim.dll`, 32 bits) fica entre o SDD e a
`monpj432.dll` do Mongoose: repassa cada chamada J2534 e registra tudo em
`C:\ProgramData\jlr_shim\capture.log`, incluindo os bytes UDS de cada
WriteMsgs/ReadMsgs.

## Compilar o shim

```
i686-w64-mingw32-gcc -shared -O2 -o monpj_shim.dll shim.c shim.def \
    -static-libgcc -Wl,--enable-stdcall-fixup
```

## Procedimento de captura

Com a ignição ligada e o carro conectado pelo Mongoose:

1. **Instalar o driver do Mongoose** (uma vez): rode `MongoosePro_JLR X32.msi`
   (pasta `Disco2`) e aprove o UAC. Isso instala a `monpj432.dll`, o driver
   de kernel e registra o Mongoose como VCI J2534.
   OBS: enquanto o driver estiver instalado, o Mongoose deixa de ser a porta
   COM (usbser) e o `car-scan` não fala com ele. Reverte-se no fim.
2. **Ativar o shim**: PowerShell como administrador →
   `./register-shim.ps1`. Ele acha a entrada do Mongoose, guarda o caminho da
   DLL real, copia o shim e aponta o registro para ele (com backup).
3. **Rodar o SDD**: selecione o Mongoose como VCI, conecte ao carro e vá ao
   editor de configuração (CCF). **Altere UMA opção de conforto** que você
   queira (ex.: ligar as luzes de cortesia dos espelhos) e grave.
   Isso já aplica a mudança de verdade pelo SDD — e o shim registra como.
4. **Reverter o shim**: `./unregister-shim.ps1` (como administrador).
5. **(Opcional) desinstalar o driver do Mongoose** para o `car-scan` voltar a
   usar a porta COM.
6. Me mande o `C:\ProgramData\jlr_shim\capture.log` (ou rode `analyze.py`):
   eu extraio a sequência exata (sessão, segurança, download/bloco, checksum)
   e implemento a gravação no `car-scan`.

## O que procurar no log

As linhas `TX ... data=...` e `RX ... data=...` para a ECU alvo (BCM, CAN
0x726/0x72E) mostram os serviços UDS: `10 02`/`10 03` (sessão), `27 ..`
(segurança), e o mecanismo de escrita do CCF (`2E`, ou `34`/`36`/`37`, ou um
DID/rotina específico). É isso que falta para fechar a gravação.
