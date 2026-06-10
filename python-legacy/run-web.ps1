# ArxivCat Web 版本启动脚本

Write-Host "=" -NoNewline -ForegroundColor Cyan
Write-Host ("=" * 59) -ForegroundColor Cyan
Write-Host "  ArxivCat Web 版本" -ForegroundColor Yellow
Write-Host "=" -NoNewline -ForegroundColor Cyan
Write-Host ("=" * 59) -ForegroundColor Cyan
Write-Host ""

# 检查是否在正确的目录
if (-not (Test-Path "web\app.py")) {
    Write-Host "错误: 请在 ArxivCat 项目根目录运行此脚本" -ForegroundColor Red
    exit 1
}

# 激活 conda web 环境
$condaPath = "D:\anaconda3"
$env:PATH = "$condaPath\envs\web;$condaPath\envs\web\Scripts;$condaPath\envs\web\Library\bin;$env:PATH"
$python = "$condaPath\envs\web\python.exe"

if (-not (Test-Path $python)) {
    Write-Host "错误: 未找到 web 环境，请先创建: conda create -n web python=3.10" -ForegroundColor Red
    exit 1
}

Write-Host "使用环境: web (conda)" -ForegroundColor Green

Write-Host "检查依赖..." -ForegroundColor Green

# 检查并安装依赖
$packages = @("flask", "flask-cors", "requests", "google-genai")
foreach ($pkg in $packages) {
    $installed = & $python -m pip show $pkg 2>$null
    if (-not $installed) {
        Write-Host "安装 $pkg..." -ForegroundColor Yellow
        & $python -m pip install $pkg -q
    }
}

Write-Host ""
Write-Host "启动服务器..." -ForegroundColor Green
Write-Host ""

# 获取本机 IP
$ip = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object {$_.InterfaceAlias -notlike "*Loopback*" -and $_.IPAddress -notlike "169.254.*"} | Select-Object -First 1).IPAddress

Write-Host "=" -NoNewline -ForegroundColor Cyan
Write-Host ("=" * 59) -ForegroundColor Cyan
Write-Host "  访问地址:" -ForegroundColor Yellow
Write-Host ""
Write-Host "  本地:     " -NoNewline -ForegroundColor White
Write-Host "http://localhost:5000" -ForegroundColor Green
Write-Host "  局域网:   " -NoNewline -ForegroundColor White
Write-Host "http://${ip}:5000" -ForegroundColor Green
Write-Host ""
Write-Host "  提示:" -ForegroundColor Yellow
Write-Host "    - 手机和电脑连接同一 WiFi" -ForegroundColor Gray
Write-Host "    - 手机浏览器访问后点击'添加到主屏幕'" -ForegroundColor Gray
Write-Host "    - 就可以像 app 一样使用了" -ForegroundColor Gray
Write-Host ""
Write-Host "  按 Ctrl+C 停止服务器" -ForegroundColor Yellow
Write-Host "=" -NoNewline -ForegroundColor Cyan
Write-Host ("=" * 59) -ForegroundColor Cyan
Write-Host ""

# 启动 Flask
Set-Location web
& $python app.py
