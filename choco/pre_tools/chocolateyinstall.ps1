$ErrorActionPreference = 'Stop'; # stop on all errors
$toolsDir   = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

$psFile = Join-Path "$(Split-Path -parent $MyInvocation.MyCommand.Definition)" 'install-logontask.ps1'
# do not use Start-ChocolateyProcessAsAdmin because it outputs a scary red wall of text.
# Start-ChocolateyProcessAsAdmin "& `'$psFile`'" *> $null
& "$psFile"
echo "`nThe program is scheduled to start on logon, or start manually with`n  Start-ScheduledTask QuakeRun`n"
