# Runs inside the Windows VM, started by vm-inno.sh: checks the pinned Inno Setup installer
# (hash, signer) and installs it for the current user into C:\tools\inno-<version>.
param(
  [Parameter(Mandatory)][string]$Installer,
  [Parameter(Mandatory)][string]$Sha256,
  [Parameter(Mandatory)][string]$Version
)
$ErrorActionPreference = 'Stop'
$Dir = "C:\tools\inno-$Version"
$Iscc = "$Dir\ISCC.exe"

# Returns true when ISCC.exe exists and the uninstaller Inno wrote reports the wanted version.
# ISCC.exe itself carries version 0.0.0.0 in its resource, so it cannot be the test.
function Test-Inno {
  $unins = "$Dir\unins000.exe"
  (Test-Path $Iscc) -and (Test-Path $unins) -and ((Get-Item $unins).VersionInfo.ProductVersion -like "$Version*")
}

# Prints one failure line and stops the script with exit code 1.
function Fail([string]$why) { Write-Output "=== FAIL inno $why"; exit 1 }

if (Test-Inno) {
  Remove-Item -Force -ErrorAction SilentlyContinue $Installer
  Write-Output "=== inno ready $Version (present)"
  exit 0
}
if ((Get-FileHash -Algorithm SHA256 $Installer).Hash -ne $Sha256.ToUpperInvariant()) { Fail 'hash' }
$sig = Get-AuthenticodeSignature $Installer
if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'CN=Pyrsys B\.V\.') {
  Fail "signature ($($sig.Status) $($sig.SignerCertificate.Subject))"
}
$iargs = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/CURRENTUSER', '/NOICONS',
  "/DIR=`"$Dir`"", '/LOG="C:\winvm\inno-install.log"')
$p = Start-Process -FilePath $Installer -ArgumentList $iargs -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail "install exit $($p.ExitCode); read C:\winvm\inno-install.log" }
if (-not (Test-Inno)) { Fail 'ISCC.exe missing or wrong version after install' }
Remove-Item -Force -ErrorAction SilentlyContinue $Installer
Write-Output "=== inno ready $Version"
exit 0
