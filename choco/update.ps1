pushd $PSScriptRoot\..
cargo build --release
$files = (
    ".\scripts\install.ps1",
    ".\scripts\uninstall.ps1",
    ".\target\release\noconsole.exe",
    ".\target\release\quakerun.exe",
    ".\target\release\switch.exe",
    ".\target\release\indexer.exe"
)
$unshim = (
    "$PSScriptRoot\tools\noconsole.exe.ignore",
    "$PSScriptRoot\tools\switch.exe.ignore",
    "$PSScriptRoot\tools\indexer.exe.ignore"
)

rm -recurse -force $PSScriptRoot\tools\ -ea ignore
new-item -type directory $PSScriptRoot\tools\ -force | out-null
copy $PSScriptRoot\pre_tools\* $PSScriptRoot\tools\ -recurse -force
copy $files $PSScriptRoot\tools
new-item $unshim  | out-null
popd