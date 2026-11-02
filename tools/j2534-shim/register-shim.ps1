#Requires -RunAsAdministrator
# Instala o shim gravador J2534 entre o SDD e a monpj432.dll do Mongoose.
# Faz backup da entrada de registro e guarda o caminho da DLL real para o shim.
# Reverter: unregister-shim.ps1

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$cfgDir = 'C:\ProgramData\jlr_shim'
New-Item -ItemType Directory -Force -Path $cfgDir | Out-Null

# Acha a entrada PassThruSupport do Mongoose (FunctionLibrary com monpj432).
$roots = @(
  'HKLM:\SOFTWARE\WOW6432Node\PassThruSupport.04.04',
  'HKLM:\SOFTWARE\PassThruSupport.04.04'
)
$entry = $null
foreach ($r in $roots) {
  if (-not (Test-Path $r)) { continue }
  Get-ChildItem $r | ForEach-Object {
    $fl = (Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue).FunctionLibrary
    if ($fl -and $fl -match 'monpj432') { $entry = $_; }
  }
  if ($entry) { break }
}
if (-not $entry) {
  Write-Host "Nenhuma entrada J2534 do Mongoose (monpj432) encontrada no registro." -ForegroundColor Red
  Write-Host "Instale primeiro o driver: 'MongoosePro_JLR X32.msi' (Disco2)." -ForegroundColor Yellow
  exit 1
}

$realPath = (Get-ItemProperty $entry.PSPath).FunctionLibrary
Write-Host "Entrada do Mongoose: $($entry.PSChildName)"
Write-Host "DLL real: $realPath"

# Backup do valor original (.reg) e guarda o caminho real para o shim.
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$bak = Join-Path $cfgDir "FunctionLibrary-backup-$stamp.txt"
$realPath | Set-Content -Encoding ASCII $bak
$realPath | Set-Content -Encoding ASCII (Join-Path $cfgDir 'real.txt')
# também guarda o nome da chave para reverter
$entry.PSPath | Set-Content -Encoding ASCII (Join-Path $cfgDir 'entry.txt')
Write-Host "Backup do caminho original: $bak"

# Copia o shim para um local estável.
$shimSrc = Join-Path $here 'monpj_shim.dll'
$shimDst = Join-Path $cfgDir 'monpj_shim.dll'
Copy-Item $shimSrc $shimDst -Force
Write-Host "Shim copiado para: $shimDst"

# Aponta o FunctionLibrary para o shim.
Set-ItemProperty $entry.PSPath -Name FunctionLibrary -Value $shimDst
Write-Host "Registro atualizado: o SDD agora passa pelo shim." -ForegroundColor Green
Write-Host "Log da captura: $cfgDir\capture.log"
Write-Host ""
Write-Host "Agora: rode o SDD, selecione o Mongoose, e faça UMA alteração de CCF." -ForegroundColor Cyan
Write-Host "Depois rode unregister-shim.ps1 para reverter." -ForegroundColor Cyan
