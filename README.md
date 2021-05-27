# Aero

**Aero** is a new modern, experimental, unix-like operating system written in Rust. Aero follows the monolithic kernel design and it is inspired by the Linux Kernel and the Zircon Kernel.

Please make sure you use the **latest nightly** of rustc before building Aero.

![workflow](https://github.com/Andy-Python-Programmer/aero/actions/workflows/build.yml/badge.svg)
[![lines_of_code](https://tokei.rs/b1/github/Andy-Python-Programmer/aero)](https://github.com/Andy-Python-Programmer/aero)
[![discord](https://img.shields.io/discord/828564770063122432)](https://discord.gg/8gwhTTZwt8)

## Screenshots
<img src="misc/os.png">

## Features
- Modern UEFI bootloader

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
$ cargo aero run
```

## Chainloading
Chainloading is a technique that allows one bootloader to call another bootloader as if the system had just booted up. Aero's bootloader has support for chainloading. Check out the [Aero Chainloading](docs/chainloading.md) docs to get more information about how to use this feature.

## Contributing
Contributions are absolutely, positively welcome and encouraged! Check out [CONTRIBUTING.md](CONTRIBUTING.md) for the contributing guidelines for aero.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
