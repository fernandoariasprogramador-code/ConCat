param(
    [switch]$Phone
)

if ($Phone) {
    $env:CONCAT_PHONE = "1"
}

# Import MSVC 2019 x64 build environment
cmd.exe /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat" && set' | ForEach-Object {
    if ($_ -match '^(.*?)=(.*)$') {
        Set-Item -Force -Path "env:$($matches[1])" -Value $matches[2]
    }
}

# Set FFmpeg and LLVM paths
$env:FFMPEG_DIR = "C:\ffmpeg"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;C:\ffmpeg\bin;$env:Path"

# Link MSVC 2022 STL compatibility library
$compatLibDir = "d:\CONCAT Video\Concat-main"
$env:LIB = "$compatLibDir;$env:LIB"

# Target directories for both quick and debug profiles
$quickDir = "d:\CONCAT Video\Concat-main\src\target\quick"
$debugDir = "d:\CONCAT Video\Concat-main\src\target\debug"

if (-not (Test-Path $quickDir)) { New-Item -ItemType Directory -Force -Path $quickDir | Out-Null }
if (-not (Test-Path $debugDir)) { New-Item -ItemType Directory -Force -Path $debugDir | Out-Null }

Copy-Item "C:\ffmpeg\bin\*.dll" $quickDir -Force -ErrorAction SilentlyContinue
Copy-Item "C:\ffmpeg\bin\*.dll" $debugDir -Force -ErrorAction SilentlyContinue

# Clean stale lock files if any previous build was aborted
Remove-Item "$debugDir\.cargo-lock", "$debugDir\.cargo-build-lock", "$quickDir\.cargo-lock" -Force -ErrorAction SilentlyContinue

Set-Location "d:\CONCAT Video\Concat-main\src"

if ($Phone -or $env:CONCAT_PHONE -eq "1") {
    $env:CONCAT_PHONE = "1"
    Write-Host "Modo Celular / Android (CONCAT_PHONE=1) activado."
}

Write-Host "Lanzando Concat con Gemini AI integrado..."
cargo run -p concat

