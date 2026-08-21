# Loads the MSVC x64 dev environment (cl.exe/link.exe) and points cargo at
# the x86_64-pc-windows-msvc toolchain, so `cargo build`/`test` work on a
# Windows-on-ARM64 machine that only has the default (non-ARM64) C++ Build
# Tools workload installed. See docs/DEVELOPMENT.md for why this is needed.
#
# Usage: dot-source this once per PowerShell session, then use cargo normally.
#   . .\scripts\dev-shell.ps1
#   cargo test --workspace

$ErrorActionPreference = "Stop"

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

$vcvars = Get-ChildItem "C:\BuildTools\VC\Auxiliary\Build\vcvarsall.bat", `
    "${env:ProgramFiles}\Microsoft Visual Studio\*\*\VC\Auxiliary\Build\vcvarsall.bat", `
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\*\*\VC\Auxiliary\Build\vcvarsall.bat" `
    -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName

if (-not $vcvars) {
    Write-Error "Could not find vcvarsall.bat. Install Visual Studio Build Tools (Desktop development with C++) first."
    return
}

$envDump = cmd /c "`"$vcvars`" x64 >nul 2>&1 && set"
foreach ($line in $envDump) {
    if ($line -match "^(PATH|LIB|INCLUDE|LIBPATH)=(.*)$") {
        Set-Item -Path "env:$($Matches[1])" -Value $Matches[2]
    }
}

Write-Output "x64 MSVC dev environment loaded. cargo will build via the x86_64-pc-windows-msvc override for this repo."
