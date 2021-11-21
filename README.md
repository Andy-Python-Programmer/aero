<p align="center">
    <img src="./misc/aero-logo.png">
</p>

# Aero

**Aero** is a new modern, experimental, unix-like operating system written in Rust. 
Aero follows the monolithic kernel design and it is inspired by the Linux Kernel and 
the Zircon Kernel. Aero supports *modern* PC features such as Long Mode, 5-level paging, 
and SMP (multicore), to name a few.

![workflow](https://github.com/Andy-Python-Programmer/aero/actions/workflows/build.yml/badge.svg)
[![lines_of_code](https://tokei.rs/b1/github/Andy-Python-Programmer/aero)](https://github.com/Andy-Python-Programmer/aero)
[![discord](https://img.shields.io/discord/828564770063122432)](https://discord.gg/8gwhTTZwt8)

**Is this a Linux distribution?**
No, Aero runs its own kernel that does *not* originate from Linux and does not share any source code or binaries with the Linux kernel.

**Official Discord Server**: <https://discord.gg/8gwhTTZwt8>

# Screenshots
<img src="misc/demo.gif">
<p align="center"><i>Aero OS running in Qemu (the background image is by <a href="https://www.youtube.com/watch?v=dQw4w9WgXcQ">Rick Astley</a>)</i></p>

# Features
- 64-bit higher half kernel
- 4/5 level paging
- Preemptive per-cpu scheduler
- Modern UEFI bootloader
- ACPI support (ioapic, lapic)
- Symmetric Multiprocessing (SMP)
- On-demand paging

# Roadmap

Check out [ROADMAP.md](ROADMAP.md) for this month's roadmap.

# How to Build and Run Aero

Please make sure you have a **unix-like** host system before building 
Aero. If you are using windows, its highly recommended to use WSL 2.

## Dependencies

Before building Aero, you need the following things installed:
- `rustc` should be the **latest nightly**
- `qemu`
- `nasm`
- `g++` 5.1 or later
- `ninja`
- `parted`
- `meson`

## Hardware

The following are *not* requirements but are *recommendations*:
- ~15GB of free disk space
- \>= 8GB RAM
- \>= 2 cores
- Internet access

Beefier machines will lead to much faster builds!

## Getting the source code

The very first step to work on Aero is to clone the repository:
```shell
$ git clone https://github.com/Andy-Python-Programmer/aero
$ cd aero
```

## What is `aero_build`?
`aero_build` is a small binary that is used to orchestrate the tooling in the Aero repository. 
It is used to build docs, run tests, and compile Aero. It is the now preferred way to build Aero and 
it replaces the old makefiles from before.

## Building Aero

**Note:** Building Aero will require a relatively large amount of storage space. You
may want to have upwards of 10 or 15 gigabytes available.

To build Aero, run `cargo aero build`. This command will build the bootloader, kernel and 
userland. The build system builds the respective packages at the following stages:

1. First we clone the the bootloader prebuilt binaries (`limine`) which is responsible for loading the kernel binary
from the disk.

2. Next we build userland (`userland/*`), the first task that it does it to clone and install 
the GCC Aero target and mlibc which can take from 20 minutes to an hour.

3. Then we build the kernel (`aero_kernel`). Since the kernel is central component of an operating
system (where the magic happens), it can take from 2 minutes to 5 minutes to compile.

After the build system has finished building all of the *subsystems* of Aero, next it assembles/packages
all of the generated binaries into an `aero.iso` file located in the `build/` directory.

## Running Aero in an emulator

After the build system has done building Aero we can straight away run the generated `aero.iso` file an emulator! 
This can be done using the `cargo aero run` command (which by default uses Qemu as the emulator and can be configured). 
This command automatically builds Aero and then runs it in the specified emulator. This means that you can straight away
run `cargo aero run` instead of running `cargo aero build` before!

## Nightly Images

Want to give Aero a shot, without building it! You can go to the [latest job](https://github.com/Andy-Python-Programmer/aero/actions/workflows/build.yml?query=is%3Asuccess+branch%3Amaster) and download the latest nightly image (`aero.img`), under artifacts.

# Contributing

Contributions are absolutely, positively welcome and encouraged! Check out [CONTRIBUTING.md](CONTRIBUTING.md) for the contributing guidelines for aero.

# License

Aero is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version. See the [LICENSE](LICENSE) file for license rights and limitations.
