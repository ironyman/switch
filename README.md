# Install

Easy install command with chocolatey (run as elevated admin)
```
cinst -y switch
```
# Run
It should run at logon, or run manually with

```
Start-ScheduledTask quakerun
```

and stop with 
```
quakerun.exe --stop
```

Open the switch UI with ``Alt+` ``
# Build
Install rust
```
iwr https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y
```
Also install VS community edition with desktop development with C++ option. This has to be done with GUI.

Install llvm, required for rocksdb
```
cinst -y llvm
```
Build this project
```
cargo build
```
We can run the build output like this
```
.\target\debug\quakerun.exe -c "$((pwd).path)\target\debug\switch.exe"
```
quakerun.exe will listen for ``Alt+` `` key press and launch switch.exe in windows terminal in quake mode.

We can copy the output to a more permanent path and start quakerun.exe on logon
```
md c:\switch
copy scripts\*,target\debug\*.exe c:\switch
c:\switch\install-logontask.ps1
```
## Inner loop
Alternatively for dev inner loop,
```
cargo build
copy .\scripts\* .\target\debug\
.\target\debug\install-logontask.ps1
```
To rebuild and restart
```
.\target\debug\quakerun.exe -s
cargo build
start-scheduledtask quakerun
```
or
```
.\target\debug\quakerun.exe -s; cargo build; .\target\debug\quakerun.exe
```
To uninstall
```
c:\switch\uninstall-logontask.ps1
del -recurse c:\switch
```

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
or
```
choco install -y switch*.nupkg
```
# Check install status
```
Get-ScheduledTask -taskpath \ -TaskName quakerun | select -ExpandProperty Actions
```

Logs are here

```
$env:APPDATA\switch\quake_terminal_runner.log
$env:APPDATA\switch\switch.log
```