# Builds Horizon nightly-2026-11-13 (9f42064) with the mods below applied, incrementally.
# 1) horizon-mixins plan+apply into a fresh stage, 2) copy only changed files into the kept
# stage (so cargo's incremental cache survives), 3) cargo build --profile fast.
# Output exe: .build-tools\eksen-target\fast\horizon-client.exe   Log: .build-tools\eksen-build.log
$ErrorActionPreference = 'Stop'
$c = Split-Path $PSScriptRoot -Parent   # cmod = the folder that holds .build-tools
$bt = "$c\.build-tools"
$r = "$env:APPDATA\horizon-launcher\rust\1.99.1-x86_64-pc-windows-gnu"
$env:PATH = "$r\bin;$r\mingw64\bin;$env:PATH"
$env:CARGO_HOME = "$env:APPDATA\horizon-launcher\cargo"
$hm = "$env:APPDATA\horizon-launcher\native\tools\5a64af938ec8\bin\horizon-mixins.exe"
$bundles = @{
  eksen          = "$c\Eksen\mixins\eksen"
  hatlayers      = "$c\HatLayers\horizon\mixins\hatlayers"
  shulkerboxtooltip = "$c\ShulkerTooltip\mixins\shulkerboxtooltip"
  noreloadscreen = "$c\NoReloadScreen\mixins\noreloadscreen"
  betterhitreg   = "$c\BetterHitreg\horizon\mixins\betterhitreg"
  ozgurkamera    = "$c\OzgurKamera\horizon\mixins\ozgurkamera"
  freelook       = "$c\Freelook\horizon\mixins\freelook"
  zoomify        = "$c\Zoomify\horizon\mixins\zoomify"
  voicechat      = "$c\VoiceChat\mixins\voicechat"
  olderservers   = "$c\OlderServers\horizon\mixins\olderservers"
  healthindicators = "$c\HealthIndicators\horizon\mixins\healthindicators"
  opsec          = "$c\OpSec\horizon\mixins\opsec"
}
$mods = "$bt\build-mods"
if (Test-Path $mods) { Remove-Item -Recurse -Force $mods }
New-Item -ItemType Directory $mods | Out-Null
foreach ($k in $bundles.Keys) { Copy-Item -Recurse $bundles[$k] "$mods\$k" }
# A new folder each run: Windows can keep a just-deleted stage locked (E_IO, os error 5).
Get-ChildItem $bt -Directory -Filter 'stage-new*' | ForEach-Object { Remove-Item -Recurse -Force $_.FullName -ErrorAction SilentlyContinue }
$fresh = "$bt\stage-new-$(Get-Date -Format yyyyMMddHHmmss)"
$ErrorActionPreference = 'Continue'
& $hm apply --workspace "$bt\horizon-src" --mods $mods --out $fresh 2>&1 | Out-String | Write-Output
if ($LASTEXITCODE -ne 0) { Write-Output "APPLY FAILED"; exit 1 }
node -e @"
const fs=require('fs'),path=require('path');
const A=process.argv[1],B=process.argv[2];
function walk(d,o=[]){for(const e of fs.readdirSync(d,{withFileTypes:true})){if(e.name==='target'||e.name==='.git')continue;const p=path.join(d,e.name);if(e.isDirectory())walk(p,o);else o.push(p)}return o}
let n=0;for(const f of walk(A)){const rel=path.relative(A,f),g=path.join(B,rel),a=fs.readFileSync(f);
if(!fs.existsSync(g)||!a.equals(fs.readFileSync(g))){fs.mkdirSync(path.dirname(g),{recursive:true});fs.writeFileSync(g,a);n++;console.log('synced',rel)}}
console.log('changed files:',n);
"@ $fresh "$bt\eksen-stage"
$env:CARGO_TARGET_DIR = "$bt\eksen-target"
$log = "$bt\eksen-build.log"
$p = Start-Process -FilePath "$r\bin\cargo.exe" -ArgumentList 'build','--locked','--profile','fast','-p','horizon-client','--bin','horizon-client','-j','8' -WorkingDirectory "$bt\eksen-stage" -RedirectStandardError $log -RedirectStandardOutput "$log.out" -NoNewWindow -PassThru
try { $p.PriorityClass = 'BelowNormal' } catch {}
$p.WaitForExit()
Write-Output "build exit=$($p.ExitCode)"
