all: run

pull:
	git pull --recurse-submodules
	git submodule sync --recursive
	git submodule update --recursive --init

test:
	cargo test

build:
	cargo build

run:
	cargo run

clean:
	cargo clean

init:
	touch hdd.img

preview:
	@ mkdir -p build/efi/boot
	@ mkdir -p build/efi/kernel

	@ echo !==== BUILDING AERO BOOT ====!
	@ cd src && cargo build --package aero_boot --target x86_64-unknown-uefi

	@ echo !==== BUILDING AERO KERNEL ====!
	@ cd src && cargo build --package aero_kernel

	@ echo !==== PACKAGING ====!
	
	@ cp src/target/x86_64-aero_os/debug/aero_kernel build/efi/kernel/aero.elf
	@ cp src/target/x86_64-unknown-uefi/debug/aero_boot.efi build/efi/boot/aero_boot.efi
	@ echo "\\\efi\\\boot\\\aero_boot.EFI" > build/startup.nsh

	@ cmd.exe /C qemu-system-x86_64 -drive format=raw,file=fat:rw:build/ \
		-L "C:\Program Files\qemu" \
		-bios bundled/ovmf/OVMF-pure-efi.fd \
		-machine q35 \
		-drive if=pflash,format=raw,file=bundled/ovmf/OVMF_CODE-pure-efi.fd \
		-drive if=pflash,format=raw,file=bundled/ovmf/OVMF_VARS-pure-efi.fd
