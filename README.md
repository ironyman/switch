# Install

Easy install command with chocolatey (run as elevated admin)
```
cinst -y switch
```

# Build
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

Alternatively for dev inner loop,
```
cargo build
copy scripts\* targets\debug\
.\targets\debug\install.ps1
```
To build and restart
```
cargo build
.\targets\debug\quakerun.exe -s
start-scheduledtask quakerun
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

# Package for chocolatey

```
cd choco
.\update.ps1
choco pack
```

And to install the packaged package

```
cinst -y switch -source (pwd).Path
```

# Uninstall

```
& "$env:USERPROFILE\OneDrive - Microsoft\bin\switch\uninstall.ps1"
del -recurse "$env:USERPROFILE\OneDrive - Microsoft\bin\switch\
```
