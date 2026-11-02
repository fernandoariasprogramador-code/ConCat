@echo off
title Concat Modo Celular (Android)
cd /d "d:\CONCAT Video\Concat-main"
echo ========================================================
echo  Iniciando Concat en Modo Celular (Android / CONCAT_PHONE)
echo  Dimensiones: 420x880 (Aspect ratio de smartphone vertical)
echo ========================================================
powershell -ExecutionPolicy Bypass -File ".\run_app.ps1" -Phone
pause
