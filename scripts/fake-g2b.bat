@echo off
rem Double-click launcher for fake-g2b.ps1: builds, then opens the dev window, fake G2B server and Edge.
rem ASCII only - cmd reads this file in the console code page, not UTF-8.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fake-g2b.ps1" %*
if errorlevel 1 pause
