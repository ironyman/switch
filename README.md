# Setup
Install rust
```
iwr https://win.rustup.rs/x86_64 -OutFile rustup‑init.exe
rustup‑init.exe -y
```
Build this project
```
cargo build
md "$env:USERPROFILE\OneDrive - Microsoft\bin\switch"
copy scripts\*,target\debug\*.exe "$env:USERPROFILE\OneDrive - Microsoft\bin\switch"
& "$env:USERPROFILE\OneDrive - Microsoft\bin\switch\install.ps1"
```

# Run
It should run at logon, or run manually with

```
Start-ScheduledTask quakerun
```

and stop with 
```
& "$env:USERPROFILE\OneDrive - Microsoft\bin\switch\quakerun.exe --stop
```

Open the switch UI with ``Alt+` ``

# Uninstall

```
& "$env:USERPROFILE\OneDrive - Microsoft\bin\switch\uninstall.ps1"
del -recurse "$env:USERPROFILE\OneDrive - Microsoft\bin\switch\
```
