$psFile = Join-Path "$(Split-Path -parent $MyInvocation.MyCommand.Definition)" 'uninstall.ps1'
# Start-ChocolateyProcessAsAdmin "& `'$psFile`'" *> $null
& "$psFile" | out-null