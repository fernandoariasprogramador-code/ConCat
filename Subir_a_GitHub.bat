@echo off
title Subir Concat a GitHub
cd /d "d:\CONCAT Video\Concat-main"
echo ========================================================
echo  Subiendo solucion y cambios a:
echo  https://github.com/fernandoariasprogramador-code/ConCat.git
echo ========================================================
echo.
git push -u origin main
echo.
echo ========================================================
echo  Proceso finalizado.
echo ========================================================
pause
