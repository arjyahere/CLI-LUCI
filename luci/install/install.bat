@echo off
setlocal

echo ========================================
echo   Luci CLI Installer for Windows
echo ========================================
echo.

set "INSTALL_DIR=C:\luci"

REM Create installation directory
if not exist "%INSTALL_DIR%" (
    mkdir "%INSTALL_DIR%"
)

REM Copy binaries from build output
set "BUILD_DIR=%~dp0..\target\release"
if exist "%BUILD_DIR%\luci.exe" (
    echo Installing luci and lucid to %INSTALL_DIR%...
    copy /Y "%BUILD_DIR%\luci.exe" "%INSTALL_DIR%\luci.exe" >nul
    copy /Y "%BUILD_DIR%\lucid.exe" "%INSTALL_DIR%\lucid.exe" >nul
) else (
    echo Build not found. Run 'cargo build --release' first.
    pause
    exit /b 1
)

REM Add to user PATH (no admin needed)
echo Adding to PATH...
for /f "tokens=2*" %%A in ('reg query "HKCU\Environment" /v Path 2^>nul') do set "OLDPATH=%%B"
echo %OLDPATH% | findstr /I /C:"%INSTALL_DIR%" >nul
if %errorLevel% neq 0 (
    setx PATH "%OLDPATH%;%INSTALL_DIR%"
    echo Added %INSTALL_DIR% to PATH
) else (
    echo Already in PATH
)

echo.
echo ========================================
echo   Installation Complete!
echo ========================================
echo.
echo   Binary location: %INSTALL_DIR%
echo.
echo   Restart your terminal, then run:
echo     luci status
echo     lucid
echo.
pause
