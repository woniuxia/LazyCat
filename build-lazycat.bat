@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" -arch=x64
set PATH=C:\Strawberry\perl\bin;%PATH%
cd /d E:\Projects\LazyCat
pnpm --filter @lazycat/desktop build:tauri
