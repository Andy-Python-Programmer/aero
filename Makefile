jinx:
	if [ ! -f "target/jinx" ]; then \
		curl -Lo target/jinx https://github.com/mintsuki/jinx/raw/30e7d5487bff67a66dfba332113157a08a324820/jinx; \
		chmod +x target/jinx; \
	fi

	# FIXME: autosync
	mkdir -p target/cargo-home
	cp build-support/rust/config.toml target/cargo-home/config.toml

.PHONY: distro
distro: jinx
	./target/jinx build-all

SOURCE_DIR := src
USERLAND_DIR := userland
USERLAND_TARGET := builds/userland/target/init
KERNEL_TARGET := src/target/x86_64-unknown-none/release/aero_kernel

.PHONY: clean
clean:
	rm -rf src/target

$(KERNEL_TARGET): $(shell find $(SOURCE_DIR) -type f -not -path '$(SOURCE_DIR)/target/*')
	cd src && cargo build --package aero_kernel --release
	./build-support/mkiso.sh

$(USERLAND_TARGET): $(shell find $(USERLAND_DIR) -type f -not -path '$(USERLAND_DIR)/target/*')
	./target/jinx rebuild userland
	@$(MAKE) distro-image

.PHONY: iso
iso: $(KERNEL_TARGET)

.PHONY: distro-image
distro-image: distro
	./build-support/mkimage.sh

.PHONY: qemu
qemu: $(KERNEL_TARGET) $(USERLAND_TARGET)
	${QEMU_PATH}/qemu-system-x86_64 -cdrom target/aero.iso -m 8G -serial stdio --boot d -s -enable-kvm -cpu host,+vmx -drive file=target/disk.img,if=none,id=NVME1,format=raw -device nvme,drive=NVME1,serial=nvme 

.PHONY: doc
doc:
	cd src && cargo doc --package aero_kernel --release --target-dir=../target/doc/
	cp web/index.html target/doc/index.html
ifeq ($(open),yes)
	xdg-open target/doc/index.html
endif
