# Import MSVC 2019 x64
cmd.exe /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat" && set' | ForEach-Object {
    if ($_ -match '^(.*?)=(.*)$') {
        Set-Item -Force -Path "env:$($matches[1])" -Value $matches[2]
    }
}
$env:Path = "C:\Program Files\LLVM\bin;$env:Path"

& clang-cl /c /O2 /GS- "d:\CONCAT Video\Concat-main\msvc_vector_algorithms.cpp" /Fo"d:\CONCAT Video\Concat-main\msvc_vector_algorithms.obj"
