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

# Ensure runtime DLLs are available in target dir
$targetDir = "d:\CONCAT Video\Concat-main\src\target\quick"
if (-not (Test-Path $targetDir)) {
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
}
Copy-Item "C:\ffmpeg\bin\*.dll" $targetDir -Force -ErrorAction SilentlyContinue

Set-Location "d:\CONCAT Video\Concat-main\src"

Write-Host "Running cargo check..."
cargo check -p concat
