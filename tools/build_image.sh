#!/bin/bash

# Copyright (C) 2021 The Aero Project Developers.
#
# This file is part of The Aero Project.
#
# Aero is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# Aero is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with Aero. If not, see <https://www.gnu.org/licenses/>.

SPATH=$(dirname $(readlink -f "$0"))
AERO_PATH=$(realpath $SPATH/..)

AERO_BUILD=$AERO_PATH/build
AERO_BUNDLED=$AERO_PATH/bundled
AERO_SYSROOT=$AERO_PATH/sysroot/aero
AERO_SRC=$AERO_PATH/src
AERO_KERNEL_TARGET=$AERO_PATH/src/target/x86_64-aero_os

set -x -e

usage() {
    echo -e "usage: $0 [--bios] [--efi]"
}

bios=
efi=

while getopts ber:h arg
do
    case $arg in
        b) bios=1;;
        e) efi=1;;
    esac

    shift
done

if [ ! -d $AERO_BUNDLED/limine ]; then
    git clone --branch=v2.0-branch-binary --depth=1 https://github.com/limine-bootloader/limine.git $AERO_BUNDLED/limine
fi

if [ -d $AERO_BUILD ]; then
    sudo rm -rf $AERO_BUILD
fi

mkdir $AERO_BUILD
# Create the sysroot directory if it already does not exist since the sysroot
# directory is only created when the sysroot is built (which is optional) and is
# required for building the initramfs gzip archive.
mkdir -p $AERO_SYSROOT
dd if=/dev/zero bs=2M count=0 seek=64 of=$AERO_BUILD/aero.img

parted -s $AERO_BUILD/aero.img mklabel gpt
parted -s $AERO_BUILD/aero.img mkpart primary 2048s 100%

if [ -d $AERO_BUILD/mnt ]; then
    sudo rm -rf $AERO_BUILD/mnt
fi

mkdir $AERO_BUILD/mnt

sudo losetup -Pf --show $AERO_BUILD/aero.img > loopback_dev

if [ "$bios" ]; then
    sudo mkfs.ext2 `cat loopback_dev`p1
    sudo mount `cat loopback_dev`p1 $AERO_BUILD/mnt
else
    sudo mkfs.fat -F 32 `cat loopback_dev`p1
    sudo mount `cat loopback_dev`p1 $AERO_BUILD/mnt
fi

pushd $AERO_SYSROOT
find . | cpio -H newc -o >$AERO_BUILD/initramfs.cpio
popd

sudo mkdir $AERO_BUILD/mnt/boot

if [[ -z "${RELEASE}" ]]; then
    sudo cp $AERO_KERNEL_TARGET/debug/aero_kernel $AERO_BUILD/mnt/boot/aero.elf
else
    sudo cp $AERO_KERNEL_TARGET/release/aero_kernel $AERO_BUILD/mnt/boot/aero.elf
fi

sudo cp $AERO_BUILD/initramfs.cpio $AERO_BUILD/mnt/
sudo cp $AERO_SRC/.cargo/limine.cfg $AERO_BUILD/mnt/
sudo cp $AERO_BUNDLED/limine/limine.sys $AERO_BUILD/mnt/boot/

if [ "$efi" ]; then
    sudo mkdir -p $AERO_BUILD/mnt/EFI/BOOT/
    sudo cp $AERO_BUNDLED/limine/BOOTX64.EFI $AERO_BUILD/mnt/EFI/BOOT/
fi

sync

sudo umount $AERO_BUILD/mnt
sudo losetup -d `cat loopback_dev`

rm -rf $AERO_BUILD/mnt/ loopback_dev

$AERO_BUNDLED/limine/limine-install-linux-x86_64 $AERO_BUILD/aero.img
