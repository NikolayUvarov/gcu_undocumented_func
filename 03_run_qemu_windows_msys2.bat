@echo off
rem Sound: AC97 card for audio_gw through DirectSound; VirtIO network card; RDRAND for the TLS and key services.
echo Starting MIND CORE in QEMU...

rem Look for QEMU in the standard MSYS2 folders (UCRT64 or MinGW64)
set QEMU_PATH=C:\msys64\ucrt64\bin\qemu-system-x86_64.exe
if not exist "%QEMU_PATH%" set QEMU_PATH=C:\msys64\mingw64\bin\qemu-system-x86_64.exe

if not exist "%QEMU_PATH%" (
    echo ERROR: QEMU not found in C:\msys64!
    echo Check where MSYS2 was installed.
    pause
    exit /b
)

"%QEMU_PATH%" -bios OVMF.fd -drive format=raw,file=fat:rw:usb_root -m 512 -smp 4,sockets=1,cores=4,threads=1 -serial stdio -rtc base=localtime -cpu qemu64,+rdrand -audiodev dsound,id=snd0 -device AC97,audiodev=snd0 -nic user,model=virtio-net-pci
