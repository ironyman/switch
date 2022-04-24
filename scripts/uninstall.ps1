$scriptRoot = "";

$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    echo "Need to be admin"
    break
}

Unregister-ScheduledTask -TaskName QuakeRun -Confirm:$False -ea silentlycontinue

if (Test-Path -Path "$PSScriptRoot\quakerun.exe") {
    if (get-process quakerun -ea silentlycontinue) {
        & "$PSScriptRoot\quakerun.exe" -s
    }
    echo "Delete files to complete uninstall"
}
