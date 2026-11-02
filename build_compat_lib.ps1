# Import MSVC 2019 x64
cmd.exe /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat" && set' | ForEach-Object {
    if ($_ -match '^(.*?)=(.*)$') {
        Set-Item -Force -Path "env:$($matches[1])" -Value $matches[2]
    }
}
$env:Path = "C:\Program Files\LLVM\bin;$env:Path"

Write-Host "Compiling compat sources..."
& clang-cl /c /std:c++17 /O2 /GS- -I"d:\CONCAT Video\Concat-main" "d:\CONCAT Video\Concat-main\stl_compat.cpp" /Fo"d:\CONCAT Video\Concat-main\stl_compat.obj"
& clang-cl /c /std:c++17 /O2 /GS- -I"d:\CONCAT Video\Concat-main" "d:\CONCAT Video\Concat-main\xcharconv_ryu_tables.cpp" /Fo"d:\CONCAT Video\Concat-main\xcharconv_ryu_tables.obj"
& clang-cl /c /std:c++17 /O2 /GS- -I"d:\CONCAT Video\Concat-main" "d:\CONCAT Video\Concat-main\xcharconv_tables_double.cpp" /Fo"d:\CONCAT Video\Concat-main\xcharconv_tables_double.obj"
& clang-cl /c /std:c++17 /O2 /GS- -I"d:\CONCAT Video\Concat-main" "d:\CONCAT Video\Concat-main\xcharconv_tables_float.cpp" /Fo"d:\CONCAT Video\Concat-main\xcharconv_tables_float.obj"

Write-Host "Creating stl_compat.lib..."
& lib.exe /OUT:"d:\CONCAT Video\Concat-main\stl_compat.lib" "d:\CONCAT Video\Concat-main\stl_compat.obj" "d:\CONCAT Video\Concat-main\xcharconv_ryu_tables.obj" "d:\CONCAT Video\Concat-main\xcharconv_tables_double.obj" "d:\CONCAT Video\Concat-main\xcharconv_tables_float.obj"
