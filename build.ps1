$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

uv run pyinstaller --noconfirm --clean --windowed --onedir --name FloatNote `
    --icon assets/floatnote.ico --add-data "assets;assets" `
    --distpath build/out app.py
if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed with exit code $LASTEXITCODE" }

# 只更新程序文件，保留 dist\FloatNote 下用户自己的 notes/config/history
$staging = "build/out/FloatNote"
New-Item -ItemType Directory -Force -Path "dist/FloatNote/_internal" | Out-Null
Copy-Item "$staging/FloatNote.exe" "dist/FloatNote/FloatNote.exe" -Force
robocopy "$staging/_internal" "dist/FloatNote/_internal" /MIR /NFL /NDL /NJH /NJS | Out-Null
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with exit code $LASTEXITCODE" }

# zip 只打包程序，不含用户数据
$zip = "dist/FloatNote-portable.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $staging -DestinationPath $zip

Write-Output "Portable build: dist\FloatNote\FloatNote.exe"
Write-Output "Zip: $zip"