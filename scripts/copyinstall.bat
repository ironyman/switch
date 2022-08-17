@echo on
robocopy %~dp0\ %userprofile%\appdata\roaming\switch
powershell %userprofile%\appdata\roaming\switch\install-logontask.ps1
powershell start-scheduledtask -taskname quakerun
echo done
pause