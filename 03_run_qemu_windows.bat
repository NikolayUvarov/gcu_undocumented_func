@echo off
echo Starting the OS in QEMU (with UART/COM port support)...
qemu-system-x86_64.exe -bios OVMF.fd -drive format=raw,file=fat:rw:usb_root -m 512 -smp 4,sockets=1,cores=4,threads=1 -serial stdio -rtc base=localtime
pause
