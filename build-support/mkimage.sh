IMAGE_PATH=target/disk.img

./target/jinx sysroot

rm -rf $IMAGE_PATH

dd if=/dev/zero of=$IMAGE_PATH bs=1G count=0 seek=512
parted -s $IMAGE_PATH mklabel gpt
parted -s $IMAGE_PATH mkpart primary 2048s 100%

# ensure loop kernel module is enabled
if ! lsmod | grep -q 'loop'; then
    echo 'mkimage.sh: `loop` kernel module not found, attempting to load'
    sudo modprobe loop
fi

sudo losetup -Pf --show $IMAGE_PATH > loopback_dev
sudo mkfs.ext2 `cat loopback_dev`p1 -I128

rm -rf target/disk_image/
mkdir target/disk_image
sudo mount `cat loopback_dev`p1 target/disk_image
sudo cp -r -v sysroot/. target/disk_image/
pushd target/disk_image
sudo mkdir dev proc tmp
popd
sync
sudo umount target/disk_image/
sudo losetup -d `cat loopback_dev`
sync

rm -rf loopback_dev
rm -rf target/disk_image
