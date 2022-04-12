$scriptRoot = "";

Unregister-ScheduledTask -TaskName QuakeRun -Confirm:$False -ea silentlycontinue

if (Test-Path -Path "$PSScriptRoot\quakerun.exe") {
    if (get-process quakerun.exe -ea silentlycontinue) {
        & "$PSScriptRoot\quakerun.exe" -s
    }
    echo "Delete files to complete uninstall"
}
