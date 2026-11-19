# Waits for the running Horizon client to close on its own, then installs the
# freshly built exe into the active version and checks the copy. Never stops the game.
$src = "C:\Users\Murat\Desktop\cmod\.build-tools\eksen-target\fast\horizon-client.exe"
$dst = "$env:APPDATA\horizon-launcher\versions\nightly-2026-11-13-9f42064\horizon-client.exe"
$log = "C:\Users\Murat\Desktop\cmod\.build-tools\install-when-closed.log"
"$(Get-Date -Format s) waiting for horizon-client to close" | Out-File -Encoding utf8 $log
while (Get-Process -Name horizon-client -ErrorAction SilentlyContinue) { Start-Sleep -Seconds 5 }
Start-Sleep -Seconds 2
try {
    Copy-Item -Force $src $dst -ErrorAction Stop
    $a = (Get-FileHash $src).Hash; $b = (Get-FileHash $dst).Hash
    if ($a -eq $b) { "$(Get-Date -Format s) installed ($a)" | Out-File -Append -Encoding utf8 $log }
    else { "$(Get-Date -Format s) copy hash mismatch" | Out-File -Append -Encoding utf8 $log }
} catch {
    "$(Get-Date -Format s) install failed: $($_.Exception.Message)" | Out-File -Append -Encoding utf8 $log
}
