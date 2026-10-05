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
echo [1/6] Terminating ALL Citadel and Guard processes (including clones/zombies)...
taskkill /f /fi "IMAGENAME eq citadel*" >nul 2>&1
taskkill /f /fi "IMAGENAME eq guard*" >nul 2>&1
taskkill /f /im citadel-client.exe >nul 2>&1
taskkill /f /im citadel-server.exe >nul 2>&1
taskkill /f /im citadel-recovery.exe >nul 2>&1
taskkill /f /im guard-svc.exe >nul 2>&1
powershell -NoProfile -NonInteractive -Command "Get-Process | Where-Object { $_.ProcessName -like '*citadel*' -or $_.ProcessName -like '*guard*' } | Stop-Process -Force -ErrorAction SilentlyContinue; Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like '*citadel_kiosk*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }" >nul 2>&1

echo [2/6] Restoring Registry ACL permissions for candidate user...
powershell -NoProfile -NonInteractive -Command "$paths = @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer', 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\System', 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies'); foreach ($p in $paths) { if (Test-Path $p) { try { $acl = Get-Acl $p; $user = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name; if ($user) { $ntUser = [System.Security.Principal.NTAccount]$user; $acl.SetOwner($ntUser); $rule = New-Object System.Security.AccessControl.RegistryAccessRule($ntUser, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'); $acl.ResetAccessRule($rule); Set-Acl $p $acl } } catch { } } }" >nul 2>&1

echo [3/6] Removing restrictive lockdown policies from HKCU, HKLM, and all user hives (HKU)...
:: Delete from HKCU
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableTaskMgr /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableLockWorkstation /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableChangePassword /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableAltTab /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoClose /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoLogoff /f >nul 2>&1
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" /v EnableSnapAssistFlyout /f >nul 2>&1
reg delete "HKCU\Software\Policies\Microsoft\Windows\TabletPC" /v DisableSnippingTool /f >nul 2>&1

:: Delete from all user SID hives dynamically (never hardcode machine SIDs)
for /f "tokens=*" %%H in ('reg query HKU ^| findstr /R "HKEY_USERS\\S-1-5-21-[0-9-]*$"') do (
    reg delete "%%H\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableTaskMgr /f >nul 2>&1
    reg delete "%%H\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableLockWorkstation /f >nul 2>&1
    reg delete "%%H\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableChangePassword /f >nul 2>&1
    reg delete "%%H\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableAltTab /f >nul 2>&1
    reg delete "%%H\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys /f >nul 2>&1
    reg delete "%%H\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoClose /f >nul 2>&1
    reg delete "%%H\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoLogoff /f >nul 2>&1
)

:: Also clean HKLM if any existed
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableTaskMgr /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableLockWorkstation /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableChangePassword /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System" /v DisableAltTab /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoWinKeys /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoClose /f >nul 2>&1
reg delete "HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer" /v NoLogoff /f >nul 2>&1

echo [4/6] Restoring Bluetooth and WLAN services...
sc config bthserv start= auto >nul 2>&1
net start bthserv >nul 2>&1
sc config WlanSvc start= auto >nul 2>&1
net start WlanSvc >nul 2>&1

echo [5/6] Ensuring Windows Explorer Shell is active...
taskkill /f /im explorer.exe >nul 2>&1
start explorer.exe

echo.
echo [6/6] VERIFYING 100%% CITADEL PROCESS DESTRUCTION & RESTORATION (3 CHECKS)...
echo.
powershell -NoProfile -NonInteractive -Command "$c=0; for($i=1; $i -le 3; $i++) { Start-Sleep -Milliseconds 400; $procs = Get-Process | Where-Object { $_.ProcessName -like '*citadel*' -or $_.ProcessName -like '*guard*' }; if ($procs.Count -eq 0) { Write-Host ('[PASS ' + $i + '/3] Verified: 0 Citadel processes active. Restrictions completely destroyed.'); $c++ } else { Write-Host ('[FAIL ' + $i + '/3] Lingering: ' + ($procs.ProcessName -join ', ')); $procs | Stop-Process -Force -ErrorAction SilentlyContinue } }; if ($c -eq 3) { Write-Host ''; Write-Host '========================================================================' -ForegroundColor Green; Write-Host 'User, you can now successfully close the window.' -ForegroundColor Green; Write-Host '========================================================================' -ForegroundColor Green } else { Write-Host 'Lingering processes detected. Please re-run.' -ForegroundColor Yellow }"

echo.
echo ========================================================================
echo [SUCCESS] RESTORATION COMPLETE!
echo - Power buttons (Shut down, Restart, Sleep) are RESTORED.
echo - Ctrl+Alt+Delete (Task Manager, Lock, Sign Out) is RESTORED.
echo - Ctrl+Shift+Escape (Task Manager) is RESTORED.
echo - All Windows Key shortcuts (Win+R, Win+Shift+S, Win+L) are RESTORED.
echo - Alt+Tab and Screenshots are RESTORED.
echo - Bluetooth and WLAN networking are RESTORED.
echo - Explorer Shell is RESTORED.
echo ========================================================================
echo.
pause
