# XXX: fucking hell andy. you had a reminder to not commit this
# file. you did. you fucking did. you're a fucking idiot. move
# this shit to aero.py.

rm -rf disk.img

dd if=/dev/zero bs=1M count=0 seek=512 of=disk.img
parted -s disk.img mklabel gpt
parted -s disk.img mkpart primary 2048s 100%
sudo losetup -Pf --show disk.img > loopback_dev
sudo mkfs.ext2 `cat loopback_dev`p1
rm -rf disk_image/
mkdir disk_image
sudo mount `cat loopback_dev`p1 disk_image
sudo cp aero.py disk_image/aero.py
sync
sudo umount disk_image/
sudo losetup -d `cat loopback_dev`
sync
rm -rf loopback_dev
