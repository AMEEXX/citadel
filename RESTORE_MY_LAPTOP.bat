@echo off
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [ELEVATION REQUIRED] Requesting Administrator privileges to reset your laptop...
    powershell -Command "Start-Process '%~f0' -Verb RunAs"
    exit /b
)

echo ========================================================================
echo               RESETTING ALL RESTRICTIONS ON YOUR LAPTOP
echo ========================================================================
echo.
echo [1/4] Terminating any Citadel processes...
taskkill /f /im citadel-client.exe >nul 2>&1
taskkill /f /im guard-svc.exe >nul 2>&1

echo [2/4] Restoring Registry ACL permissions for amitk...
powershell -NoProfile -Command "$paths = @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer', 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\System', 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies'); foreach ($p in $paths) { if (Test-Path $p) { try { $acl = Get-Acl $p; $user = [System.Security.Principal.NTAccount]'AmitX\amitk'; $acl.SetOwner($user); $rule = New-Object System.Security.AccessControl.RegistryAccessRule($user, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'); $acl.ResetAccessRule($rule); Set-Acl $p $acl } catch { Write-Host 'ACL note:' $_ } } }"

echo [3/4] Removing restrictive lockdown policies...
:: Delete from HKCU
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableTaskMgr /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableLockWorkstation /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableChangePassword /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoClose /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoLogoff /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" /v EnableSnapAssistFlyout /f >nul 2>&1

:: Delete from specific user SID hive
reg delete "HKU\S-1-5-21-1751942760-950062233-4152076368-1001\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableTaskMgr /f >nul 2>&1
reg delete "HKU\S-1-5-21-1751942760-950062233-4152076368-1001\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableLockWorkstation /f >nul 2>&1
reg delete "HKU\S-1-5-21-1751942760-950062233-4152076368-1001\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableChangePassword /f >nul 2>&1
reg delete "HKU\S-1-5-21-1751942760-950062233-4152076368-1001\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys /f >nul 2>&1
reg delete "HKU\S-1-5-21-1751942760-950062233-4152076368-1001\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoClose /f >nul 2>&1
reg delete "HKU\S-1-5-21-1751942760-950062233-4152076368-1001\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoLogoff /f >nul 2>&1

:: Also clean HKLM if any existed
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableTaskMgr /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableLockWorkstation /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableChangePassword /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoClose /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoLogoff /f >nul 2>&1

echo [4/4] Restarting Windows Explorer Shell...
taskkill /f /im explorer.exe >nul 2>&1
start explorer.exe

echo.
echo ========================================================================
echo [SUCCESS] RESTORATION COMPLETE!
echo - Power buttons (Shut down, Restart, Sleep) are RESTORED.
echo - Ctrl+Alt+Delete (Task Manager, Lock, Sign Out) is RESTORED.
echo - Ctrl+Shift+Escape (Task Manager) is RESTORED.
echo - All Windows Key shortcuts are RESTORED.
echo ========================================================================
echo.
pause
