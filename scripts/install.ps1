$scriptRoot = "";

if (Test-Path -Path "$PSScriptRoot\quakerun.exe") {
    $scriptRoot = $PSScriptRoot;
} elseif (Test-Path -Path "$((pwd).path)\quakerun.exe") {
    $scriptRoot = (pwd).path;
} elseif (Test-Path -Path "$PSScriptRoot\..\quakerun.exe") {
    $scriptRoot = (Resolve-path -Path $PSScriptRoot\..).Path;
}

$taskAction = New-ScheduledTaskAction `
    -Execute "$scriptRoot\noconsole.exe" `
    -Argument "`"`"`"$scriptRoot\quakerun.exe`"`"`" -c `"`"`"$scriptRoot\switch.exe`"`"`""

$taskTrigger = New-ScheduledTaskTrigger -atlogon -User $env:USERNAME

$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Highest

Register-ScheduledTask `
    -TaskName QuakeRun `
    -Action $taskAction `
    -Trigger $taskTrigger `
    -principal $principal `
    -Description "Start switch.exe with quakerun.exe at logon" `
    -force

