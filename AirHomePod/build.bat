@echo off
setlocal enabledelayedexpansion

echo ============================================================
echo   AirHomePod: Tek Bir Bagimsiz EXE (Single-File) Paketleme
echo ============================================================
echo.

:: 0. Arac Kontrolleri (Rust & .NET)
where cargo >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [DIKKAT] 'cargo' (Rust Derleyicisi) bulunamadi!
    echo Lutfen Rust'i yukleyin: https://rustup.rs/
    echo.
    pause
    exit /b 1
)

where dotnet >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [DIKKAT] '.NET SDK' bulunamadi!
    echo Lutfen .NET 8 SDK'yi yukleyin: https://dotnet.microsoft.com/download/dotnet/8.0
    echo.
    pause
    exit /b 1
)

:: 1. Rust Ses ve Protokol Motorunu Derle
echo [1/2] Rust Cekirdek Motoru (airplay_core.dll) optimize ediliyor...
cd "%~dp0crates\airplay-core"
cargo build --release
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [HATA] Rust derlemesi basarisiz oldu!
    echo.
    pause
    exit /b %ERRORLEVEL%
)

:: 2. Tum Projeyi Tek Bir .EXE Icine Gom ve Sikistir (Single-File Publish)
echo.
echo [2/2] Tum bagimliliklar ve motor tek bir AirHomePod.exe icine paketleniyor...
cd "%~dp0apps\AirHomePod.Tray"
dotnet publish -c Release -r win-x64 --self-contained true /p:PublishSingleFile=true /p:IncludeNativeLibrariesForSelfExtract=true /p:EnableCompressionInSingleFile=true -o "%~dp0dist"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [HATA] Tek dosya .EXE paketleme basarisiz oldu!
    echo.
    pause
    exit /b %ERRORLEVEL%
)

:: CLI test aracini da opsiyonel olarak kopyala
cd "%~dp0"
copy /Y "crates\airplay-core\target\release\airplay-cli.exe" "dist\" >nul 2>&1

echo.
echo ============================================================
echo   Tebrikler! Tum Proje TEK BIR .EXE Olarak Sikistirildi!
echo.
echo   Dosya Konumu: dist\AirHomePod.exe
echo.
echo   Bu AirHomePod.exe dosyasini istediginiz yere tasiyabilir,
echo   hicbir ek DLL veya .NET kurulumuna gerek olmadan 
echo   tek basina dogrudan calistirabilirsiniz.
echo ============================================================
echo.
pause
