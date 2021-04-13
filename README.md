# Areo

**Aero** is a new modern, unix based operating system written in Rust and is being developed for educational purposes. Aero follows the monolithic kernel design and it is inspired by the Linux Kernel and the Zircon Kernel.

Please make sure you use the **latest nightly** of rustc before building Aero.

![workflow](https://github.com/Andy-Python-Programmer/aero/actions/workflows/build.yml/badge.svg)
[![lines_of_code](https://tokei.rs/b1/github/Andy-Python-Programmer/aero)](https://github.com/Andy-Python-Programmer/aero)
[![discord](https://img.shields.io/discord/828564770063122432)](https://discord.gg/8gwhTTZwt8)

## Screenshots
<img src="misc/os.png">

## Roadmap

Check out [ROADMAP.md](ROADMAP.md) for this month's roadmap.

## Building Aero

### Prerequisites
- The nightly [rust compiler](https://www.rust-lang.org/).
- [qemu](https://www.qemu.org/)
- [nasm](https://nasm.us)

### Build
To build and run aero:

```sh
$ cargo run
```

## Contributing
Contributions are absolutely, positively welcome and encouraged! Check out [CONTRIBUTING.md](CONTRIBUTING.md) for the contributing guidelines for aero.

## License
The source code in this project is licensed under the Apache License 2.

Here are some exceptions:
- The bundled/ovmf directory is licensed under BSD license
