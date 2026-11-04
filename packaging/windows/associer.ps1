# Associe Varium aux fichiers, pour l'utilisateur courant, sans installateur :
# à lancer depuis le dossier décompressé qui contient varium.exe
# (clic droit → « Exécuter avec PowerShell »). « -Retirer » défait l'association.
#
# Varium apparaît alors dans « Ouvrir avec » pour .json (fichiers Varium),
# .docx, .odt, .md, .txt, .xlsx, .ods, .csv, .odp et .pptx ; .varium s'ouvre
# directement dans Varium. L'installateur (varium-windows-x86_64-setup.exe)
# fait la même chose.
param([switch]$Retirer)
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'varium.exe'
$classes = 'HKCU:\Software\Classes'
$extensions = '.json', '.docx', '.odt', '.md', '.txt', '.xlsx', '.ods', '.csv', '.odp', '.pptx'

if ($Retirer) {
    Remove-Item "$classes\Varium.Fichier" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$classes\Applications\varium.exe" -Recurse -ErrorAction SilentlyContinue
    foreach ($e in $extensions) {
        Remove-ItemProperty "$classes\$e\OpenWithProgids" -Name 'Varium.Fichier' -ErrorAction SilentlyContinue
    }
    Write-Host 'Associations de Varium retirées.'
    exit
}
if (-not (Test-Path $exe)) { throw "varium.exe introuvable à côté de ce script ($exe)." }
$commande = "`"$exe`" `"%1`""
New-Item "$classes\Varium.Fichier\shell\open\command" -Force | Set-ItemProperty -Name '(default)' -Value $commande
Set-ItemProperty "$classes\Varium.Fichier" -Name '(default)' -Value 'Fichier Varium'
New-Item "$classes\Varium.Fichier\DefaultIcon" -Force | Set-ItemProperty -Name '(default)' -Value "$exe,0"
New-Item "$classes\Applications\varium.exe\shell\open\command" -Force | Set-ItemProperty -Name '(default)' -Value $commande
Set-ItemProperty "$classes\Applications\varium.exe" -Name 'FriendlyAppName' -Value 'Varium'
New-Item "$classes\.varium" -Force | Set-ItemProperty -Name '(default)' -Value 'Varium.Fichier'
foreach ($e in $extensions) {
    New-Item "$classes\$e\OpenWithProgids" -Force | New-ItemProperty -Name 'Varium.Fichier' -PropertyType None -Value ([byte[]]@()) -Force | Out-Null
    New-Item "$classes\Applications\varium.exe\SupportedTypes" -Force | New-ItemProperty -Name $e -Value '' -Force | Out-Null
}
# Prévenir l'Explorateur.
Add-Type -Namespace Win32 -Name Shell -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int e, int f, IntPtr a, IntPtr b);'
[Win32.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
Write-Host 'Varium est proposé dans « Ouvrir avec » pour ses formats.'
