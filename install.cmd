@echo off
setlocal
set "INSTALL_DIR=%LOCALAPPDATA%\AgentDemo\bin"
set "SOURCE=%~dp0agent-demo.exe"

if not exist "%SOURCE%" (
  where cargo >nul 2>nul || (
    echo agent-demo.exe is not beside install.cmd and Cargo is unavailable.
    echo Download the release package or install Rust, then try again.
    exit /b 1
  )
  echo Building Agent Demo...
  cargo build --release --manifest-path "%~dp0Cargo.toml" || exit /b 1
  set "SOURCE=%~dp0target\release\agent-demo.exe"
)

if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%" || exit /b 1
copy /y "%SOURCE%" "%INSTALL_DIR%\agent-demo.exe" >nul || exit /b 1

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command ^
  "$d=[Environment]::ExpandEnvironmentVariables('%INSTALL_DIR%'); $p=[Environment]::GetEnvironmentVariable('Path','User'); $parts=@($p -split ';' | Where-Object { $_ -and $_ -ne $d }); [Environment]::SetEnvironmentVariable('Path',(($parts + $d) -join ';'),'User')" || exit /b 1

echo.
echo Agent Demo installed successfully.
echo Command: agent-demo
echo Open a new terminal after setup so PATH is refreshed.
echo.
"%INSTALL_DIR%\agent-demo.exe" config
endlocal
