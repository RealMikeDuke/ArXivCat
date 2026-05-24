# ArxivCat build script
# Tkinter only.

$ErrorActionPreference = 'Stop'
$ProjectRoot = $PSScriptRoot
$EntryPoint  = "$ProjectRoot\main.py"
$ExeName     = "ArxivCat"
$Version     = "v0.7.1"
$ReleaseName = "$ExeName-$Version-win64"
$PythonExe   = "D:\anaconda3\envs\arxivcat\python.exe"
$EnvRoot     = Split-Path $PythonExe -Parent
$TclDir      = "$EnvRoot\Library\lib\tcl8.6"
$TkDir       = "$EnvRoot\Library\lib\tk8.6"
$TclDll      = "$EnvRoot\Library\bin\tcl86t.dll"
$TkDll       = "$EnvRoot\Library\bin\tk86t.dll"
$RuntimeHook = "$ProjectRoot\pyi_rth_tk_env.py"
$SpecFile    = "$ProjectRoot\ArxivCat.spec"
$BuildDir    = "$ProjectRoot\build"
$DistDir     = "$ProjectRoot\dist"

Write-Host "==> building $ExeName.exe" -ForegroundColor Cyan
Write-Host "    python: $PythonExe" -ForegroundColor DarkGray

if (-not (Test-Path $PythonExe)) {
    Write-Host "`n==> build failed" -ForegroundColor Red
    Write-Host "    missing python executable: $PythonExe" -ForegroundColor Red
    exit 1
}

foreach ($path in @($TclDir, $TkDir, $TclDll, $TkDll, $RuntimeHook)) {
    if (-not (Test-Path $path)) {
        Write-Host "`n==> build failed" -ForegroundColor Red
        Write-Host "    missing required path: $path" -ForegroundColor Red
        exit 1
    }
}

try {
    & $PythonExe -m PyInstaller --version | Out-Null
} catch {
    Write-Host "    installing pyinstaller..." -ForegroundColor Yellow
    & $PythonExe -m pip install pyinstaller
}

foreach ($path in @($BuildDir, $DistDir, $SpecFile)) {
    if (Test-Path $path) {
        Remove-Item $path -Recurse -Force
    }
}

& $PythonExe -m PyInstaller `
    --clean `
    --noconfirm `
    --onefile `
    --windowed `
    --name $ExeName `
    --runtime-hook $RuntimeHook `
    --add-data "arxivcat;arxivcat" `
    --add-data "$TclDir;_tcl_data" `
    --add-data "$TkDir;_tk_data" `
    --add-binary "$TclDll;." `
    --add-binary "$TkDll;." `
    --hidden-import arxivcat `
    --hidden-import arxivcat.core `
    --hidden-import arxivcat.presenter `
    --hidden-import arxivcat.ui `
    --hidden-import arxivcat.ui.base `
    --hidden-import arxivcat.ui.tkinter_ui `
    --collect-all requests `
    --collect-all google `
    --collect-all openai `
    --collect-all fitz `
    --collect-all pymupdf `
    --hidden-import tkinter `
    --hidden-import tkinter.ttk `
    $EntryPoint

if ($LASTEXITCODE -eq 0) {
    $exe = "$ProjectRoot\dist\$ExeName.exe"
    $zip = "$ProjectRoot\dist\$ReleaseName.zip"
    if (Test-Path $zip) {
        Remove-Item $zip -Force
    }
    Compress-Archive -Path $exe -DestinationPath $zip
    $sizeMB = [math]::Round((Get-Item $exe).Length / 1MB, 1)
    Write-Host "`n==> done" -ForegroundColor Green
    Write-Host "    output: $exe" -ForegroundColor Green
    Write-Host "    zip:    $zip" -ForegroundColor Green
    Write-Host "    size:   $sizeMB MB" -ForegroundColor Green
} else {
    Write-Host "`n==> build failed" -ForegroundColor Red
    exit 1
}
