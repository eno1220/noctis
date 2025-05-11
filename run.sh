#!/bin/bash

cd loader && cargo build && cd ../kernel && cargo build && cd ..
mkdir -p mnt/EFI/BOOT/
cp loader/target/x86_64-unknown-uefi/debug/loader.efi mnt/EFI/BOOT/BOOTX64.EFI
cp kernel/target/x86_64-unknown-kernel/debug/kernel mnt/kernel.elf
qemu-system-x86_64 -m 2048M -bios thirdparty/RELEASEX64_OVMF.fd -drive format=raw,file=fat:rw:mnt -machine q35 -serial mon:stdio -nographic -no-reboot
