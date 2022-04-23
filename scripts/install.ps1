$scriptRoot = "";

$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    echo "Need to be admin"
    break
}

if (Test-Path -Path "$PSScriptRoot\quakerun.exe") {
    $scriptRoot = $PSScriptRoot;
} elseif (Test-Path -Path "$((pwd).path)\quakerun.exe") {
    $scriptRoot = (pwd).path;
} elseif (Test-Path -Path "$PSScriptRoot\..\target\debug\quakerun.exe") {
    $scriptRoot = (Resolve-path -Path $PSScriptRoot\..\target\debug\).Path;
}

$taskAction = New-ScheduledTaskAction `
    -Execute "$scriptRoot\noconsole.exe" `
    -Argument "`"`"`"$scriptRoot\quakerun.exe`"`"`" -c `"`"`"$scriptRoot\switch.exe`"`"`""

$taskTrigger = New-ScheduledTaskTrigger -atlogon -User $env:USERNAME

# Required to set_foreground_window_terminal on high integrity level process windows.
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Highest

Register-ScheduledTask `
    -TaskName QuakeRun `
    -Action $taskAction `
    -Trigger $taskTrigger `
    -principal $principal `
    -Description "Start switch.exe with quakerun.exe at logon" `
    -force

