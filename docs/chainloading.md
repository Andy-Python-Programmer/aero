# Chainloading

Chainloading is a technique that allows one bootloader to call another bootloader as if the system had just booted up. Aero's bootloader has support for chainloading. The advantage of using a chainloader is that it presents a menu which you can use to select the OS you'd like to boot from Here are some of the popular chainloaders that Aero supports:

- [GNU Grub](https://www.gnu.org/software/grub/) - [docs](#GNU-grub)

## GNU Grub
**Note**: Using GRUB requires a unix like developement enviornment. If using windows then [WSL](https://docs.microsoft.com/en-us/windows/wsl/install-win10) is recommended.

First of all, we need to install grub2:
```shell
$ sudo apt install build-essential grub-common xorriso
```

Next, we are going create the ISO file using:
```shell
$ grub-mkrescue -o aero_grub.iso iso
```

Next, we are going run Aero with the ISO file we just created:
```shell
# Note: The chainloader argument takes the generated ISO file as the argument.

$ cargo boot run --chainloader aero_grub.iso
```