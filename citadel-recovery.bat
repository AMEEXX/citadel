@echo off
echo ========================================================================
echo                 CITADEL KIOSK EMERGENCY RECOVERY UTILITY
echo ========================================================================
echo.
echo Restoring Windows desktop shell, Task Manager, and system policies...
echo.

REM 1. Re-enable Task Manager and system security options
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableTaskMgr /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableLockWorkstation /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableChangePassword /f >nul 2>&1

REM 2. Re-enable Windows Shell keys, Shut Down, and Logoff
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoClose /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoLogoff /f >nul 2>&1

REM 3. Re-enable Windows 11 Snap flyouts
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" /v EnableSnapAssistFlyout /f >nul 2>&1

REM 4. Restart Windows Explorer Shell
taskkill /f /im explorer.exe >nul 2>&1
start explorer.exe

echo [SUCCESS] CITADEL policies removed and Windows Explorer restarted.
echo System returned to normal desktop state.
echo ========================================================================
pause
