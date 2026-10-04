@echo off
rem Sound: AC97 card for audio_gw through DirectSound; VirtIO network card; RDRAND for the TLS and key services.
echo Starting the OS in QEMU (with UART/COM port support)...
qemu-system-x86_64.exe -bios OVMF.fd -drive format=raw,file=fat:rw:usb_root -m 512 -smp 4,sockets=1,cores=4,threads=1 -serial stdio -rtc base=localtime -cpu qemu64,+rdrand -audiodev dsound,id=snd0 -device AC97,audiodev=snd0 -nic user,model=virtio-net-pci
pause
