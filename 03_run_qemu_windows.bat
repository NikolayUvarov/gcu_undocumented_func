@echo off
echo Запуск ОС в QEMU (с поддержкой UART/COM-порта)...
qemu-system-x86_64.exe -bios OVMF.fd -drive format=raw,file=fat:rw:usb_root -m 512 -smp 4,sockets=1,cores=4,threads=1 -serial stdio -rtc base=localtime
pause
