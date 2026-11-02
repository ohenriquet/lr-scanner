/*
 * Shim J2534 gravador para o Mongoose Pro JLR.
 *
 * Fica entre o SDD e a monpj432.dll real: exporta as 14 funções J2534,
 * carrega a DLL real e repassa cada chamada, registrando os argumentos
 * (em especial os PASSTHRU_MSG de WriteMsgs/ReadMsgs, que carregam os bytes
 * UDS) num arquivo de log. Serve para capturar a sequência exata com que o
 * SDD grava o CCF, e então replicá-la.
 *
 * Config (texto, uma coisa por linha) em C:\ProgramData\jlr_shim\:
 *   real.txt  -> caminho completo da monpj432.dll real
 *   (o log vai para C:\ProgramData\jlr_shim\capture.log)
 *
 * Compilar 32 bits (mingw i686):
 *   i686-w64-mingw32-gcc -shared -O2 -o monpj_shim.dll shim.c shim.def \
 *       -static-libgcc -Wl,--enable-stdcall-fixup
 */

#include <windows.h>
#include <stdio.h>
#include <stdint.h>

#define CFG_DIR   "C:\\ProgramData\\jlr_shim"
#define CFG_REAL  CFG_DIR "\\real.txt"
#define LOG_PATH  CFG_DIR "\\capture.log"

typedef struct {
    unsigned long ProtocolID, RxStatus, TxFlags, Timestamp, DataSize, ExtraDataIndex;
    unsigned char Data[4128];
} PASSTHRU_MSG;

static HMODULE g_real = NULL;
static CRITICAL_SECTION g_lock;
static int g_inited = 0;

/* ---- ponteiros para as funções reais ---- */
typedef long (__stdcall *fnOpen)(void*, unsigned long*);
typedef long (__stdcall *fnClose)(unsigned long);
typedef long (__stdcall *fnConnect)(unsigned long, unsigned long, unsigned long, unsigned long, unsigned long*);
typedef long (__stdcall *fnDisconnect)(unsigned long);
typedef long (__stdcall *fnReadMsgs)(unsigned long, PASSTHRU_MSG*, unsigned long*, unsigned long);
typedef long (__stdcall *fnWriteMsgs)(unsigned long, PASSTHRU_MSG*, unsigned long*, unsigned long);
typedef long (__stdcall *fnStartPeriodic)(unsigned long, PASSTHRU_MSG*, unsigned long*, unsigned long);
typedef long (__stdcall *fnStopPeriodic)(unsigned long, unsigned long);
typedef long (__stdcall *fnStartFilter)(unsigned long, unsigned long, PASSTHRU_MSG*, PASSTHRU_MSG*, PASSTHRU_MSG*, unsigned long*);
typedef long (__stdcall *fnStopFilter)(unsigned long, unsigned long);
typedef long (__stdcall *fnSetVoltage)(unsigned long, unsigned long, unsigned long);
typedef long (__stdcall *fnReadVersion)(unsigned long, char*, char*, char*);
typedef long (__stdcall *fnGetLastError)(char*);
typedef long (__stdcall *fnIoctl)(unsigned long, unsigned long, void*, void*);

static fnOpen         r_Open;
static fnClose        r_Close;
static fnConnect      r_Connect;
static fnDisconnect   r_Disconnect;
static fnReadMsgs     r_ReadMsgs;
static fnWriteMsgs    r_WriteMsgs;
static fnStartPeriodic r_StartPeriodic;
static fnStopPeriodic  r_StopPeriodic;
static fnStartFilter  r_StartFilter;
static fnStopFilter   r_StopFilter;
static fnSetVoltage   r_SetVoltage;
static fnReadVersion  r_ReadVersion;
static fnGetLastError r_GetLastError;
static fnIoctl        r_Ioctl;

static void shimlog(const char *fmt, ...) {
    FILE *f = fopen(LOG_PATH, "a");
    if (!f) return;
    SYSTEMTIME st; GetLocalTime(&st);
    fprintf(f, "%02d:%02d:%02d.%03d ", st.wHour, st.wMinute, st.wSecond, st.wMilliseconds);
    va_list ap; va_start(ap, fmt); vfprintf(f, fmt, ap); va_end(ap);
    fputc('\n', f); fclose(f);
}

static void log_msg(const char *tag, PASSTHRU_MSG *m) {
    if (!m) { shimlog("  %s (null)", tag); return; }
    FILE *f = fopen(LOG_PATH, "a");
    if (!f) return;
    fprintf(f, "  %s proto=%lu rx=%08lx tx=%08lx size=%lu data=", tag,
            m->ProtocolID, m->RxStatus, m->TxFlags, m->DataSize);
    unsigned long n = m->DataSize; if (n > 4128) n = 4128;
    for (unsigned long i = 0; i < n; i++) fprintf(f, "%02X", m->Data[i]);
    fputc('\n', f); fclose(f);
}

static void ensure_real(void) {
    EnterCriticalSection(&g_lock);
    if (!g_inited) {
        g_inited = 1;
        char path[1024] = {0};
        FILE *f = fopen(CFG_REAL, "r");
        if (f) { if (fgets(path, sizeof(path), f)) { char *p = path; while (*p) { if (*p=='\r'||*p=='\n') { *p=0; break; } p++; } } fclose(f); }
        if (path[0]) g_real = LoadLibraryA(path);
        shimlog("=== shim carregado; real=%s handle=%p ===", path, (void*)g_real);
        if (g_real) {
            r_Open = (fnOpen)GetProcAddress(g_real, "PassThruOpen");
            r_Close = (fnClose)GetProcAddress(g_real, "PassThruClose");
            r_Connect = (fnConnect)GetProcAddress(g_real, "PassThruConnect");
            r_Disconnect = (fnDisconnect)GetProcAddress(g_real, "PassThruDisconnect");
            r_ReadMsgs = (fnReadMsgs)GetProcAddress(g_real, "PassThruReadMsgs");
            r_WriteMsgs = (fnWriteMsgs)GetProcAddress(g_real, "PassThruWriteMsgs");
            r_StartPeriodic = (fnStartPeriodic)GetProcAddress(g_real, "PassThruStartPeriodicMsg");
            r_StopPeriodic = (fnStopPeriodic)GetProcAddress(g_real, "PassThruStopPeriodicMsg");
            r_StartFilter = (fnStartFilter)GetProcAddress(g_real, "PassThruStartMsgFilter");
            r_StopFilter = (fnStopFilter)GetProcAddress(g_real, "PassThruStopMsgFilter");
            r_SetVoltage = (fnSetVoltage)GetProcAddress(g_real, "PassThruSetProgrammingVoltage");
            r_ReadVersion = (fnReadVersion)GetProcAddress(g_real, "PassThruReadVersion");
            r_GetLastError = (fnGetLastError)GetProcAddress(g_real, "PassThruGetLastError");
            r_Ioctl = (fnIoctl)GetProcAddress(g_real, "PassThruIoctl");
        }
    }
    LeaveCriticalSection(&g_lock);
}

#define REAL(f) (ensure_real(), r_##f)

long __stdcall PassThruOpen(void *name, unsigned long *dev) {
    long rc = REAL(Open) ? r_Open(name, dev) : -1;
    shimlog("PassThruOpen -> rc=%ld dev=%lu", rc, dev ? *dev : 0);
    return rc;
}
long __stdcall PassThruClose(unsigned long dev) {
    long rc = REAL(Close) ? r_Close(dev) : -1;
    shimlog("PassThruClose dev=%lu -> %ld", dev, rc);
    return rc;
}
long __stdcall PassThruConnect(unsigned long dev, unsigned long proto, unsigned long flags, unsigned long baud, unsigned long *chan) {
    long rc = REAL(Connect) ? r_Connect(dev, proto, flags, baud, chan) : -1;
    shimlog("PassThruConnect dev=%lu proto=%lu flags=%08lx baud=%lu -> rc=%ld chan=%lu", dev, proto, flags, baud, rc, chan ? *chan : 0);
    return rc;
}
long __stdcall PassThruDisconnect(unsigned long chan) {
    long rc = REAL(Disconnect) ? r_Disconnect(chan) : -1;
    shimlog("PassThruDisconnect chan=%lu -> %ld", chan, rc);
    return rc;
}
long __stdcall PassThruReadMsgs(unsigned long chan, PASSTHRU_MSG *msg, unsigned long *num, unsigned long to) {
    long rc = REAL(ReadMsgs) ? r_ReadMsgs(chan, msg, num, to) : -1;
    unsigned long n = (rc == 0 && num) ? *num : 0;
    shimlog("PassThruReadMsgs chan=%lu -> rc=%ld num=%lu", chan, rc, n);
    for (unsigned long i = 0; i < n && i < 16; i++) log_msg("RX", &msg[i]);
    return rc;
}
long __stdcall PassThruWriteMsgs(unsigned long chan, PASSTHRU_MSG *msg, unsigned long *num, unsigned long to) {
    unsigned long n = num ? *num : 0;
    shimlog("PassThruWriteMsgs chan=%lu num=%lu to=%lu", chan, n, to);
    for (unsigned long i = 0; i < n && i < 16; i++) log_msg("TX", &msg[i]);
    long rc = REAL(WriteMsgs) ? r_WriteMsgs(chan, msg, num, to) : -1;
    shimlog("  -> rc=%ld num=%lu", rc, num ? *num : 0);
    return rc;
}
long __stdcall PassThruStartPeriodicMsg(unsigned long chan, PASSTHRU_MSG *msg, unsigned long *id, unsigned long interval) {
    long rc = REAL(StartPeriodic) ? r_StartPeriodic(chan, msg, id, interval) : -1;
    shimlog("PassThruStartPeriodicMsg chan=%lu interval=%lu -> rc=%ld", chan, interval, rc);
    log_msg("PER", msg);
    return rc;
}
long __stdcall PassThruStopPeriodicMsg(unsigned long chan, unsigned long id) {
    long rc = REAL(StopPeriodic) ? r_StopPeriodic(chan, id) : -1;
    shimlog("PassThruStopPeriodicMsg chan=%lu id=%lu -> %ld", chan, id, rc);
    return rc;
}
long __stdcall PassThruStartMsgFilter(unsigned long chan, unsigned long type, PASSTHRU_MSG *mask, PASSTHRU_MSG *pat, PASSTHRU_MSG *flow, unsigned long *id) {
    long rc = REAL(StartFilter) ? r_StartFilter(chan, type, mask, pat, flow, id) : -1;
    shimlog("PassThruStartMsgFilter chan=%lu type=%lu -> rc=%ld id=%lu", chan, type, rc, id ? *id : 0);
    log_msg("MASK", mask); log_msg("PATT", pat); log_msg("FLOW", flow);
    return rc;
}
long __stdcall PassThruStopMsgFilter(unsigned long chan, unsigned long id) {
    long rc = REAL(StopFilter) ? r_StopFilter(chan, id) : -1;
    shimlog("PassThruStopMsgFilter chan=%lu id=%lu -> %ld", chan, id, rc);
    return rc;
}
long __stdcall PassThruSetProgrammingVoltage(unsigned long dev, unsigned long pin, unsigned long volt) {
    long rc = REAL(SetVoltage) ? r_SetVoltage(dev, pin, volt) : -1;
    shimlog("PassThruSetProgrammingVoltage dev=%lu pin=%lu volt=%lu -> %ld", dev, pin, volt, rc);
    return rc;
}
long __stdcall PassThruReadVersion(unsigned long dev, char *fw, char *dll, char *api) {
    long rc = REAL(ReadVersion) ? r_ReadVersion(dev, fw, dll, api) : -1;
    shimlog("PassThruReadVersion -> rc=%ld fw=%s dll=%s api=%s", rc, fw?fw:"", dll?dll:"", api?api:"");
    return rc;
}
long __stdcall PassThruGetLastError(char *desc) {
    long rc = REAL(GetLastError) ? r_GetLastError(desc) : -1;
    shimlog("PassThruGetLastError -> %s", desc ? desc : "");
    return rc;
}
long __stdcall PassThruIoctl(unsigned long chan, unsigned long ioctl, void *in, void *out) {
    long rc = REAL(Ioctl) ? r_Ioctl(chan, ioctl, in, out) : -1;
    shimlog("PassThruIoctl chan=%lu ioctl=%lu -> %ld", chan, ioctl, rc);
    return rc;
}

BOOL WINAPI DllMain(HINSTANCE h, DWORD reason, LPVOID res) {
    (void)h; (void)res;
    if (reason == DLL_PROCESS_ATTACH) { InitializeCriticalSection(&g_lock); }
    else if (reason == DLL_PROCESS_DETACH) { DeleteCriticalSection(&g_lock); }
    return TRUE;
}
