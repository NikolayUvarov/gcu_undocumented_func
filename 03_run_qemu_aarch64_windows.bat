@echo off
setlocal
rem MIND CORE for aarch64 on QEMU's virt machine, on Windows. Build it first in WSL or Linux: ARCH=aarch64 ./02_build.sh
rem   03_run_qemu_aarch64_windows.bat          the build in aarch64_root\ as a FAT disk
rem   03_run_qemu_aarch64_windows.bat image    the USB image dist\mind-core-usb-aarch64.img (./04_make_usb_image_aarch64.sh)
rem                                            as a USB stick; QEMU writes nothing to the file
rem QEMU: QEMU=<path to qemu-system-aarch64.exe>, else MSYS2 (UCRT64, MinGW64) or C:\Program Files\qemu; the firmware is
rem the edk2-aarch64-code.fd QEMU ships (MIND_AAVMF_CODE=<file> for another). The shell is in this window.
rem Paths are relative to this folder: vvfat takes the directory after the last colon, so no drive letter.
cd /d "%~dp0"

set "QEMU_EXE=%QEMU%"
if not defined QEMU_EXE if exist "C:\msys64\ucrt64\bin\qemu-system-aarch64.exe" set "QEMU_EXE=C:\msys64\ucrt64\bin\qemu-system-aarch64.exe"
if not defined QEMU_EXE if exist "C:\msys64\mingw64\bin\qemu-system-aarch64.exe" set "QEMU_EXE=C:\msys64\mingw64\bin\qemu-system-aarch64.exe"
if not defined QEMU_EXE if exist "C:\Program Files\qemu\qemu-system-aarch64.exe" set "QEMU_EXE=C:\Program Files\qemu\qemu-system-aarch64.exe"
if not defined QEMU_EXE for /f "delims=" %%Q in ('where qemu-system-aarch64.exe 2^>nul') do if not defined QEMU_EXE set "QEMU_EXE=%%Q"
if not defined QEMU_EXE goto no_qemu
for %%Q in ("%QEMU_EXE%") do set "QEMU_DIR=%%~dpQ"

rem The firmware: share\ next to the installer's .exe, or ..\share\qemu\ in MSYS2.
set "CODE=%MIND_AAVMF_CODE%"
if not defined CODE if exist "%QEMU_DIR%share\edk2-aarch64-code.fd" set "CODE=%QEMU_DIR%share\edk2-aarch64-code.fd"
if not defined CODE if exist "%QEMU_DIR%..\share\qemu\edk2-aarch64-code.fd" set "CODE=%QEMU_DIR%..\share\qemu\edk2-aarch64-code.fd"
if not defined CODE goto no_firmware
for %%F in ("%CODE%") do set "VARS_TEMPLATE=%%~dpFedk2-arm-vars.fd"
set "VARS=%TEMP%\mind-aarch64-vars.fd"
set "FLASH=-drive if=pflash,format=raw,readonly=on,file="%CODE%""
if exist "%VARS_TEMPLATE%" copy /y "%VARS_TEMPLATE%" "%VARS%" >nul && set "FLASH=%FLASH% -drive if=pflash,format=raw,file="%VARS%""

if /i "%~1"=="image" goto image
if not exist "aarch64_root\EFI\BOOT\BOOTAA64.EFI" goto no_build
set "DISK=-drive format=raw,file=fat:rw:aarch64_root"
goto run

:image
set "IMAGE=dist\mind-core-usb-aarch64.img"
if not exist "%IMAGE%" goto no_image
set "DISK=-drive "if=none,id=stick,format=raw,snapshot=on,file=%IMAGE%" -device qemu-xhci -device usb-storage,drive=stick,bootindex=1"

:run
echo Starting MIND CORE (aarch64) in QEMU: %QEMU_EXE%
"%QEMU_EXE%" -machine virt,gic-version=3,highmem=off -cpu max -m 512 -smp 4 -serial stdio %FLASH% %DISK% -device ramfb -netdev user,id=n0 -device virtio-net-pci,netdev=n0 -device virtio-keyboard-pci -device virtio-tablet-pci
goto end

:no_qemu
echo ERROR: qemu-system-aarch64.exe not found. Install QEMU (qemu.org, or MSYS2: pacman -S mingw-w64-ucrt-x86_64-qemu) or set QEMU=path.
goto end
:no_firmware
echo ERROR: edk2-aarch64-code.fd not found next to QEMU. Set MIND_AAVMF_CODE=path to an aarch64 UEFI firmware (64 MiB pflash).
goto end
:no_build
echo ERROR: Missing aarch64_root\EFI\BOOT\BOOTAA64.EFI. Build first in WSL or Linux: ARCH=aarch64 ./02_build.sh
goto end
:no_image
echo ERROR: Missing dist\mind-core-usb-aarch64.img. Make it first in WSL or Linux: ./04_make_usb_image_aarch64.sh
:end
pause
