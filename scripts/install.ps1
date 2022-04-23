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

# For some reason taskscheduler thinks QuakeRun is still running long after noconsole.exe or quakerun.exe exits.
# Adding -ExecutionTimeLimit 1 means taskscheduler will assume QuakeRun already exited and quakerun.exe
# has its own mechanism for ensuring only one instance is running.
$setting = New-ScheduledTaskSettingsSet -MultipleInstances Parallel -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit 1

Register-ScheduledTask `
    -TaskName QuakeRun `
    -Action $taskAction `
    -Trigger $taskTrigger `
    -Principal $principal `
    -Setting $setting `
    -Description "Start switch.exe with quakerun.exe at logon" `
    -force

