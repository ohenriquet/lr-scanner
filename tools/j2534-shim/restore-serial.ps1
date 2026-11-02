#Requires -RunAsAdministrator
# Remove o driver do fornecedor do Mongoose (dtmonpro / monpjaguar.inf) para o
# adaptador voltar a ser porta COM (usbser, CDC-ACM), que é como o car-scan fala
# com ele. Rode como administrador e depois RECONECTE o cabo do Mongoose.

$ErrorActionPreference = 'Continue'

# Acha o INF OEM do driver (classe VehiclePassThru, provedor Drew).
$inf = $null
$text = (pnputil /enum-drivers) -join "`n"
$blocks = $text -split "(?m)^(?:Published Name|Nome Publicado):\s*"
for ($i = 1; $i -lt $blocks.Count; $i++) {
  $b = $blocks[$i]
  if ($b -match 'monpjaguar|VehiclePassThru') {
    if ($b -match '^(oem\d+\.inf)') { $inf = $Matches[1]; break }
  }
}
if ($inf) {
  Write-Host "Removendo driver $inf ..."
  pnputil /delete-driver $inf /uninstall /force
} else {
  Write-Host "Driver monpjaguar não encontrado (talvez já removido)."
}

# Remove a entrada J2534 e a instância do dispositivo para re-enumerar.
Remove-Item 'HKLM:\SOFTWARE\WOW6432Node\PassThruSupport.04.04\Drew Technologies Inc. - MongoosePro JLR' -Recurse -Force -ErrorAction SilentlyContinue
Get-PnpDevice | Where-Object { $_.InstanceId -match 'VID_18E1' } | ForEach-Object {
  Write-Host "Removendo dispositivo $($_.InstanceId)"
  pnputil /remove-device $_.InstanceId 2>&1 | Out-Null
}
Write-Host ""
Write-Host "Pronto. RECONECTE o cabo USB do Mongoose." -ForegroundColor Green
Write-Host "Ele deve voltar como 'Dispositivo Serial USB (COMx)'." -ForegroundColor Green
