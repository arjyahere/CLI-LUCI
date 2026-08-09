@echo off
setlocal

set "repo_root=%~dp0.."
set "target=%repo_root%\luci\target"

if exist "%target%\" (
  rmdir /s /q "%target%"
  echo Removed "%target%"
) else (
  echo Nothing to clean: "%target%" not found
)
