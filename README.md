# Areo

Aero is a new modern, unix based operating system. It is being developed for educational purposes.

### Prerequisites
- The latest stable rust compiler.

### Build
To build and run aero:

```sh
    $ cargo bootimage
    $ qemu-system-x86_64 -drive format=raw,file=target/x86_64-aero_os/debug/bootimage-aero.bin
```