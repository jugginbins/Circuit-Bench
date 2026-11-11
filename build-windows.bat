@echo off
rem Builds Circuit Bench for Windows and puts CircuitBench.exe in the "dist" folder.
rem Needs Rust: install it once from https://rustup.rs (choose the default options).
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust is not installed. Get it from https://rustup.rs and run this script again.
  pause
  exit /b 1
)
cd /d "%~dp0"
cargo build --release
if errorlevel 1 (
  echo The build failed - see the messages above.
  pause
  exit /b 1
)
if not exist dist mkdir dist
copy /y target\release\circuit_bench.exe dist\CircuitBench.exe >nul
echo Done! Double-click dist\CircuitBench.exe to start Circuit Bench.
pause
