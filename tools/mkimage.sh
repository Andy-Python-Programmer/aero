#!/bin/bash

set -x -e

# sync the sysroot
echo "sysroot: syncing base-files"
cp -r base-files/. sysroot/system-root/

IMAGE_PATH=build/disk.img

# make the disk image
rm -rf $IMAGE_PATH

# set $(SUID_BINARY) based on installed SUID binary
if ! command -v sudo &> /dev/null
then
	echo "mkimage.sh: sudo not found, attempting to use opendoas"
	SUID_BINARY=$(which doas)
else
	SUID_BINARY=$(which sudo)
fi

dd if=/dev/zero bs=1G count=0 seek=512 of=$IMAGE_PATH
parted -s $IMAGE_PATH mklabel gpt
parted -s $IMAGE_PATH mkpart primary 2048s 100%
$(SUID_BINARY) losetup -Pf --show $IMAGE_PATH > loopback_dev
$(SUID_BINARY) mkfs.ext2 `cat loopback_dev`p1 -I128
rm -rf disk_image/
mkdir disk_image
$(SUID_BINARY) mount `cat loopback_dev`p1 disk_image
$(SUID_BINARY) cp -r -v sysroot/system-root/. disk_image/
pushd disk_image
$(SUID_BINARY) mkdir -p dev
$(SUID_BINARY) mkdir -p home
$(SUID_BINARY) mkdir -p tmp
$(SUID_BINARY) mkdir -p proc
$(SUID_BINARY) mkdir -p var
$(SUID_BINARY) mkdir -p mnt
popd
sync
$(SUID_BINARY) umount disk_image/
$(SUID_BINARY) losetup -d `cat loopback_dev`
sync
rm -rf loopback_dev
