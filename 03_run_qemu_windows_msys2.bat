@echo off
echo Запуск MIND CORE в QEMU...

rem Ищем QEMU в стандартных папках MSYS2 (UCRT64 или MinGW64)
set QEMU_PATH=C:\msys64\ucrt64\bin\qemu-system-x86_64.exe
if not exist "%QEMU_PATH%" set QEMU_PATH=C:\msys64\mingw64\bin\qemu-system-x86_64.exe

if not exist "%QEMU_PATH%" (
    echo ОШИБКА: QEMU не найден в C:\msys64! 
    echo Проверь, куда установился msys2.
    pause
    exit /b
)

"%QEMU_PATH%" -bios OVMF.fd -drive format=raw,file=fat:rw:usb_root -m 512 -serial stdio -rtc base=localtime
