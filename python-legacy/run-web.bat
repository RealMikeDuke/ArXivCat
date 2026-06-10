@echo off
chcp 65001 >nul
echo ========================================
echo   ArxivCat Web 版本
echo ========================================
echo.
echo 正在启动服务器...
echo.

cd web
D:\anaconda3\envs\web\python.exe app.py

pause
