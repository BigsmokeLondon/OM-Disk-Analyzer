@echo off
title Build the Mac versions of OM Disk Analyzer
echo This uploads OM Disk Analyzer to a PRIVATE repository in your GitHub account,
echo lets GitHub's Mac computers build the Intel and Apple Silicon installers,
echo and downloads them into the dist folder. It takes about 10 minutes.
echo.
pause
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\build_mac_on_github.ps1"
echo.
pause
