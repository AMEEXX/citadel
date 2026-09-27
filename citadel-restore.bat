@echo off
setlocal
:: Ensure we are running as Administrator
net session >nul 2>&1
if %errorLevel% == 0 (
    goto :admin
) else (
    echo Requesting administrative privileges...
    powershell -Command "Start-Process '%~0' -Verb RunAs"
    exit /B
)
:admin
echo ========================================================
echo   CITADEL EMERGENCY SYSTEM RESTORE UTILITY
echo ========================================================
echo.
echo Killing Citadel processes...
taskkill /F /IM citadel-client.exe /T >nul 2>&1
taskkill /F /IM guard-svc.exe /T >nul 2>&1

echo Restoring Registry Policies...
:: Explorer Policies
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v "NoWinKeys" /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v "NoClose" /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v "NoLogoff" /f >nul 2>&1

:: System Policies
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v "DisableTaskMgr" /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v "DisableLockWorkstation" /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v "DisableChangePassword" /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v "DisableAltTab" /f >nul 2>&1

:: TabletPC (Snipping Tool)
reg delete "HKCU\Software\Policies\Microsoft\Windows\TabletPC" /v "DisableSnippingTool" /f >nul 2>&1

echo Restoring Bluetooth and WLAN services...
sc config bthserv start= auto >nul 2>&1
net start bthserv >nul 2>&1
sc config WlanSvc start= auto >nul 2>&1
net start WlanSvc >nul 2>&1

echo Restarting Explorer...
taskkill /F /IM explorer.exe >nul 2>&1
start explorer.exe

echo.
echo ========================================================
echo   SYSTEM RESTORED SUCCESSFULLY! 
echo   All Task Manager, Win Key, and Alt-Tab restrictions 
echo   have been removed.
echo ========================================================
pause
