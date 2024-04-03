set -ex

./target/jinx host-build limine

rm -rf target/iso_root
mkdir -pv target/iso_root/boot

cp $1 target/iso_root/aero
cp build-support/limine.cfg src/.cargo/term_background.bmp target/iso_root/

# Install the limine binaries
cp host-pkgs/limine/usr/local/share/limine/limine-bios.sys target/iso_root/boot/
cp host-pkgs/limine/usr/local/share/limine/limine-bios-cd.bin target/iso_root/boot/
cp host-pkgs/limine/usr/local/share/limine/limine-uefi-cd.bin target/iso_root/boot/
mkdir -pv target/iso_root/EFI/BOOT
cp host-pkgs/limine/usr/local/share/limine/BOOT*.EFI target/iso_root/EFI/BOOT/

# Create the disk image.
xorriso -as mkisofs -b boot/limine-bios-cd.bin -no-emul-boot -boot-load-size 4 \
    -boot-info-table --efi-boot boot/limine-uefi-cd.bin -efi-boot-part \
    --efi-boot-image --protective-msdos-label target/iso_root -o target/aero.iso

# Install limine.
host-pkgs/limine/usr/local/bin/limine bios-install target/aero.iso
