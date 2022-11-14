#!/bin/bash

set -x -e

# sync the sysroot
echo "sysroot: syncing base-files"
cp -r base-files/. sysroot/system-root/

IMAGE_PATH=build/disk.img

# set $SUID_BINARY based on installed SUID binary
if [[ $(sudo -n | head -c1 | wc -c) -ne 0 ]]; then
	SUID_BINARY=$(which sudo)
else
	echo "mkimage.sh: sudo not found, attempting to use opendoas"
	SUID_BINARY=$(which doas)
fi

# make the disk image
rm -rf $IMAGE_PATH
dd if=/dev/zero bs=1G count=0 seek=512 of=$IMAGE_PATH
parted -s $IMAGE_PATH mklabel gpt
parted -s $IMAGE_PATH mkpart primary 2048s 100%

# ensure loop kernel module is enabled
if ! lsmod | grep -q 'loop'; then
	echo 'mkimage.sh: `loop` kernel module not found, attempting to load'
	$SUID_BINARY modprobe loop
fi

$SUID_BINARY losetup -Pf --show $IMAGE_PATH > loopback_dev
$SUID_BINARY mkfs.ext2 `cat loopback_dev`p1 -I128
rm -rf disk_image/
mkdir disk_image
$SUID_BINARY mount `cat loopback_dev`p1 disk_image
$SUID_BINARY cp -r -v sysroot/system-root/. disk_image/
pushd disk_image
$SUID_BINARY mkdir -p dev
$SUID_BINARY mkdir -p home
$SUID_BINARY mkdir -p tmp
$SUID_BINARY mkdir -p proc
$SUID_BINARY mkdir -p var
$SUID_BINARY mkdir -p mnt
popd
sync
$SUID_BINARY umount disk_image/


$SUID_BINARY losetup -d `cat loopback_dev`
sync
rm -rf loopback_dev
