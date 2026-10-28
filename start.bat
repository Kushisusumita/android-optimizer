@echo off
rem Android Optimizer - double-click to start. Options pass through, e.g.: start.bat check
title Android Optimizer
setlocal
set "HERE=%~dp0"
set "EXE=%HERE%android-optimizer.exe"
if not exist "%EXE%" set "EXE=%HERE%target\release\android-optimizer.exe"
if not exist "%EXE%" (
  where cargo >nul 2>nul
  if errorlevel 1 (
    echo The program is not here yet. Download the Windows ZIP from
    echo   https://github.com/Kushisusumita/android-optimizer/releases
    echo unpack it and double-click start.bat again - or install Rust from https://rustup.rs
    pause
    exit /b 1
  )
  echo Building the program once, about a minute...
  cargo build --release --manifest-path "%HERE%Cargo.toml" || (pause & exit /b 1)
)
cd /d "%HERE%"
"%EXE%" %*
echo.
pause
