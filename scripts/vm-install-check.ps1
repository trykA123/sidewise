# Install check of the Sidewise setup, run in the Windows VM by vm-install-check.sh (plan P4, atom IN4).
# Silent install, file list, registry, shortcut, --version, silent uninstall, leftovers, profile sentinels.
param(
  [Parameter(Mandatory = $true)][string]$Setup,
  [Parameter(Mandatory = $true)][string]$Version
)
$ErrorActionPreference = 'Stop'

$Key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{77DE4BF2-91BD-459E-B2DA-C9BC1FEB476E}_is1'
$App = "$env:LOCALAPPDATA\Programs\Sidewise"
$Lnk = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Sidewise.lnk"
$Desk = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Sidewise.lnk'
$Sentinel = 'p4-install-check.txt'
$Profiles = @("$env:APPDATA\Sidewise", "$env:LOCALAPPDATA\Sidewise")
$Expected = 'sidewise.exe README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE Inter-OFL.txt JetBrainsMono-OFL.txt SpaceGrotesk-OFL.txt material-LICENSE.txt unins000.exe unins000.dat'.Split(' ')
$Created = @()

# Prints the failure line for step $Step and exits 1.
function Fail($Step, $Why) {
  Write-Output "=== FAIL $Step $Why"
  exit 1
}

# Prints the success line for step $Step.
function Ok($Step) { Write-Output "=== ok $Step" }

# Returns $true when the uninstall key or the install folder exists.
function Installed { return (Test-Path $Key) -or (Test-Path $App) }

# 1. clean: nothing installed and no Sidewise running. Never uninstalls.
if ((Installed) -or (Get-Process -Name sidewise -ErrorAction SilentlyContinue)) {
  Fail 'clean' 'Sidewise is already installed for this VM user; uninstall it first'
}
Ok 'clean'

# 2. sentinels: one file in each profile folder; remember the folders this run created.
foreach ($dir in $Profiles) {
  if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null; $Created += $dir }
  Set-Content -Path (Join-Path $dir $Sentinel) -Value 'p4 install check'
}
Ok 'sentinels'

# 3. install: silent, exit code 0.
$p = Start-Process $Setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/LOG="C:\winvm\sw-install.log"' -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail 'install' "exit code $($p.ExitCode)" }
Ok 'install'

# 4. files: exactly the expected set.
$names = @(Get-ChildItem -Force $App | ForEach-Object { $_.Name })
$missing = @($Expected | Where-Object { $names -notcontains $_ })
$extra = @($names | Where-Object { $Expected -notcontains $_ })
if ($missing.Count -or $extra.Count) { Fail 'files' "missing: $($missing -join ',') extra: $($extra -join ',')" }
Ok 'files'

# 5. registry: the values P5 relies on (NC178).
$reg = Get-ItemProperty $Key
$want = @{ DisplayName = 'Sidewise'; DisplayVersion = $Version; Publisher = 'Sidewise'; InstallLocation = "$App\" }
foreach ($name in $want.Keys) {
  if ($reg.$name -ne $want[$name]) { Fail 'registry' "$name is '$($reg.$name)', want '$($want[$name])'" }
}
if (-not $reg.QuietUninstallString) { Fail 'registry' 'QuietUninstallString is empty' }
Ok 'registry'

# 6. shortcut: Start menu entry points at the exe; no desktop icon.
if (-not (Test-Path $Lnk)) { Fail 'shortcut' "$Lnk is missing" }
$target = (New-Object -ComObject WScript.Shell).CreateShortcut($Lnk).TargetPath
if ($target -ne "$App\sidewise.exe") { Fail 'shortcut' "target is '$target'" }
if (Test-Path $Desk) { Fail 'shortcut' 'a desktop shortcut exists' }
Ok 'shortcut'

# 7. version: the installed exe prints its version.
$tmp = Join-Path $env:TEMP 'sw-check-version.txt'
Start-Process "$App\sidewise.exe" -ArgumentList '--version' -RedirectStandardOutput $tmp -Wait -NoNewWindow
$text = ''
if (Test-Path $tmp) { $text = (Get-Content -Raw $tmp); Remove-Item $tmp }
$text = "$text".Trim()
if ($text -ne "sidewise $Version") { Fail 'version' "got '$text'" }
Ok 'version'

# 8. uninstall: the uninstaller re-launches itself from %TEMP%, so poll until the key and folder vanish.
Start-Process "$App\unins000.exe" -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait
for ($i = 0; $i -lt 60 -and (Installed); $i++) { Start-Sleep -Seconds 1 }
if (Installed) { Fail 'uninstall' 'the key or the folder is still there after 60 s' }
Ok 'uninstall'

# 9. leftovers: shortcut gone, profile sentinels kept (NC173); then clean up this run's files.
if (Test-Path $Lnk) { Fail 'leftovers' "$Lnk is still there" }
foreach ($dir in $Profiles) {
  if (-not (Test-Path (Join-Path $dir $Sentinel))) { Fail 'leftovers' "sentinel in $dir was deleted" }
}
foreach ($dir in $Profiles) { Remove-Item (Join-Path $dir $Sentinel) }
foreach ($dir in $Created) { Remove-Item $dir -ErrorAction SilentlyContinue }
Ok 'leftovers'

Write-Output '=== INSTALL CHECK GREEN'
exit 0
