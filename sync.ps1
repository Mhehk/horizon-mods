# Copies the Horizon ports from their project folders in cmod into this repository.
# Run it before committing: .\sync.ps1
$ErrorActionPreference = 'Stop'
$repo = $PSScriptRoot
$c = Split-Path $repo -Parent

# Each port: its folder here, and what to copy from where.
$ports = [ordered]@{
  'eksen'              = @{ from = "$c\Eksen"; items = @('README.md', 'compatibility.json', 'mixins', 'tools') }
  'shulkerboxtooltip'  = @{ from = "$c\ShulkerTooltip"; items = @('PARITY.md', 'compatibility.json', 'mixins', 'tools') }
  'noreloadscreen'     = @{ from = "$c\NoReloadScreen"; items = @('LICENSE', 'README.md', 'compatibility.json', 'mixins') }
  'hatlayers'          = @{ from = "$c\HatLayers"; items = @('LICENSE', 'horizon') }
  'ozgurkamera'        = @{ from = "$c\OzgurKamera"; items = @('LICENSE', 'horizon') }
  'freelook'           = @{ from = "$c\Freelook"; items = @('LICENSE', 'README.md', 'horizon') }
  'zoomify'            = @{ from = "$c\Zoomify"; items = @('LICENSE', 'README.md', 'horizon') }
  'betterhitreg'       = @{ from = "$c\BetterHitreg"; items = @('LICENSE', 'horizon') }
  'voicechat'          = @{ from = "$c\VoiceChat"; items = @('LICENSE', 'README.md', 'KURULUM.md', 'compatibility.json', 'horizon-mod.toml', 'assets', 'dist', 'mixins', 'tools') }
  'olderservers'       = @{ from = "$c\OlderServers"; items = @('horizon') }
  'healthindicators'   = @{ from = "$c\HealthIndicators"; items = @('README.md', 'horizon') }
  'opsec'              = @{ from = "$c\OpSec"; items = @('LICENSE', 'README.md', 'horizon') }
  'arena'              = @{ from = "$c\ArenaHud"; items = @('LICENSE', 'README.md', 'compatibility.json', 'mixins') }
  'hesaplar'           = @{ from = "$c\Hesaplar"; items = @('LICENSE', 'README.md', 'compatibility.json', 'mixins') }
}

foreach ($name in $ports.Keys) {
  $p = $ports[$name]
  $dest = Join-Path $repo $name
  if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
  New-Item -ItemType Directory $dest | Out-Null
  foreach ($item in $p.items) {
    $src = Join-Path $p.from $item
    if (Test-Path $src) { Copy-Item -Recurse $src $dest }
  }
}
# ShulkerBoxTooltip's own licence, from its upstream checkout.
$sbt = "$c\.build-tools\upstream\ShulkerBoxTooltip\LICENSE"
if (Test-Path $sbt) { Copy-Item $sbt "$repo\shulkerboxtooltip\LICENSE" }

# The build and install scripts.
New-Item -ItemType Directory -Force "$repo\build-tools" | Out-Null
foreach ($f in 'horizon-build-mods.ps1', 'install-when-closed.ps1') {
  Copy-Item "$c\.build-tools\$f" "$repo\build-tools\$f"
}
# No build output or lock files.
Get-ChildItem $repo -Recurse -Force -Include 'Cargo.lock' | Remove-Item -Force
Get-ChildItem $repo -Recurse -Force -Directory -Filter 'target' | Remove-Item -Recurse -Force
Write-Output "synced $($ports.Count) ports"
