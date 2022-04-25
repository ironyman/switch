$ErrorActionPreference = 'Stop'; # stop on all errors
$toolsDir   = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

$psFile = Join-Path "$(Split-Path -parent $MyInvocation.MyCommand.Definition)" 'install.ps1'
# do not use Start-ChocolateyProcessAsAdmin because it outputs a scary red wall of text.
# Start-ChocolateyProcessAsAdmin "& `'$psFile`'" *> $null
& "$psFile"
Start-ScheduledTask QuakeRun
