#!/bin/bash

set -x -e

# sync the sysroot
echo "sysroot: syncing base-files"
cp -r base-files/. sysroot/system-root/

IMAGE_PATH=build/disk.img

# make the disk image
rm -rf $IMAGE_PATH

dd if=/dev/zero bs=1G count=0 seek=512 of=$IMAGE_PATH
parted -s $IMAGE_PATH mklabel gpt
parted -s $IMAGE_PATH mkpart primary 2048s 100%
sudo losetup -Pf --show $IMAGE_PATH > loopback_dev
sudo mkfs.ext2 `cat loopback_dev`p1 -I128
rm -rf disk_image/
mkdir disk_image
sudo mount `cat loopback_dev`p1 disk_image
sudo cp -r -v sysroot/system-root/. disk_image/
pushd disk_image
sudo mkdir -p dev
sudo mkdir -p home
sudo mkdir -p tmp
sudo mkdir -p proc
sudo mkdir -p var
sudo mkdir -p mnt
popd
sync
sudo umount disk_image/
sudo losetup -d `cat loopback_dev`
sync
rm -rf loopback_dev
