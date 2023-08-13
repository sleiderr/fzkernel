<h1 align="center">
  <!--<a href="http://frozenpeach.org/fzboot"><img src="" alt="FrozenBoot" width="200"></a>-->
  FrozenBoot
</h1>
<br>
 <h4 align="center"><a href="https://frozenpeach.org/fzboot" target="_blank">FrozenBoot</a> is a modern, feature-rich x86 bootloader.</h4>

---

[![Contributors][contributors-shield]][contributors-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]
[![GPL v3][license-shield]][license-url]

## Table of Contents

- [Introduction](#introduction)
- [Build](#build)
- [Architecture](#architecture)
- [License](#license)
- [Contributing](#contributing)

## Introduction

FrozenBoot aims to become a stable, user-friendly and feature rich x86 bootloader. It will be usable
with custom kernels, through a simple API, or through multiboot2 support. But it can also be used as
a bootloader for most of the available Linux distributions (Ubuntu, Debian).
Not only that, but it will also provide various utilities to diagnose your system, or customize it 
through a user-friendly interface.

For now, the project is still in a very early phase, but we except to be able to boot common Linux-based
distributions soon.

## Build

### Get the source code

The main repository [`frozenpeach-dev/bootloader`](https://github.com/frozenpeach-dev/bootloader) contains all 
of the required files to build a minimal standalone version of the bootloader.

```shell
git clone https://github.com/frozenpeach-dev/bootloader.git
cd bootloader
```

### Install the latest Rust toolchain (Linux or macOS)

```shell
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
```

### Install the required components

Before this step, you might need to reload your shell environment after installing the Rust toolchain.

```shell
rustup component add llvm-tools-preview --toolchain nightly
rustup component add rust-src --toolchain nightly
```

### Build FrozenBoot

You can now build the project, by changing directory to the `build/` directory that contains all the 
necessary build tools.

```shell
cd build/
cargo build
cargo run
````

### (Optional) Install qemu

Use your package manager to install [qemu](https://www.qemu.org/download/#source) if you want to run the bootloader.

```shell
qemu-system-x86_64 -drive format=raw,file=boot.img
```

## Architecture

The repository is built with the following file structure:
- `build/`: contains the necessary tools to build the project
- `src/`: source files
  - `bios/`: BIOS-related utilities (such as SMBIOS)
  - `fs/`: File system / partition scheme code
  - `fzboot/`: Main bootloader code, contains the Kernel API
    - `mbr/`: Bootloader entry, contained in the disk MBR
    - `real/`: real-mode entry ("second stage"), loaded just after the MBR
    - `main/`: protected-mode entry, loaded after switching from real mode
    - `...`: feature-specific source files (`time`, `irq`)
  - `io/`: Input-output device management code
  - `mem/`: contains memory-management related code
  - `video/`: contains FrozenBoot's graphic code.
  - `x86/`: architecture-specifc code

## License

FrozenBoot is licensed under the terms of the GNU General Public License version 3 (GPLv3).
A version of that license is made available when cloning this repository in [LICENSE.txt](LICENSE.txt)

## Contributing

Thank you for considering contributing to the FrozenBoot project! 
We welcome all contributions – from bug reports and feature requests to code changes and documentation improvements.

Before you start, please take a moment to review our [Code of Conduct](), and make sure to check out our
[Contributing Guide]().




---


[contributors-shield]: https://img.shields.io/github/contributors/frozenpeach-dev/bootloader.svg?style=for-the-badge
[contributors-url]: https://github.com/frozenpeach-dev/bootloader/graphs/contributors
[license-shield]: https://img.shields.io/github/license/frozenpeach-dev/bootloader.svg?style=for-the-badge
[license-url]: https://github.com/frozenpeach-dev/bootloader/blob/master/LICENSE.txt
[stars-shield]: https://img.shields.io/github/stars/frozenpeach-dev/bootloader?style=for-the-badge
[stars-url]: https://github.com/frozenpeach-dev/bootloader/stargazers
[issues-shield]: https://img.shields.io/github/issues/frozenpeach-dev/bootloader?style=for-the-badge
[issues-url]: https://github.com/frozenpeach-dev/bootloader/issues
