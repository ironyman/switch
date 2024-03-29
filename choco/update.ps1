pushd $PSScriptRoot\..
cargo build --release
$files = (
    ".\scripts\install-logontask.ps1",
    ".\scripts\uninstall-logontask.ps1",
    ".\scripts\copyinstall.bat",
    ".\target\release\noconsole.exe",
    ".\target\release\quakerun.exe",
    ".\target\release\switch.exe",
    ".\target\release\indexer.exe"
# Static link instead of shipping with dlls seems more convenient famous last words
#    "$env:WINDIR\system32\msvcp140.dll",
#    "$env:WINDIR\system32\vcruntime140.dll"
)
$unshim = (
    "$PSScriptRoot\tools\noconsole.exe.ignore",
    "$PSScriptRoot\tools\switch.exe.ignore",
    "$PSScriptRoot\tools\indexer.exe.ignore"
)

rm -recurse -force $PSScriptRoot\tools\ -ea ignore
new-item -type directory $PSScriptRoot\tools\ -force | out-null
# Copy choco scripts
copy $PSScriptRoot\pre_tools\* $PSScriptRoot\tools\ -recurse -force
copy $files $PSScriptRoot\tools
new-item $unshim  | out-null
popd