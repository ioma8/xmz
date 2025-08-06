# xmz

<p>
  <a href="https://github.com/ioma8/xmz/actions/workflows/ci.yml">
    <img src="https://github.com/ioma8/xmz/actions/workflows/ci.yml/badge.svg" alt="CI">
  </a>
  <a href="https://opensource.org/licenses/MIT">
    <img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT">
  </a>
</p>

A high-performance, zero-allocation XML parser with a TUI for interactive traversal.

## Features

- **Blazing Fast:** Parses large XML files at high speed by leveraging `memmap` and `memchr`.
- **Zero-Allocation Parsing:** The core parser operates without memory allocations for maximum efficiency.
- **Interactive TUI:** Navigate XML trees interactively with a user-friendly terminal interface.
- **Statistics Mode:** Get insights into your XML structure, including tag counts and max depth.

## Usage

`xmz` expects an XML file path as its first argument. You can optionally enable TUI mode.

### TUI Mode

To explore an XML file interactively, run:

```sh
cargo run --release -- <path/to/your/file.xml> --tui
```

### Stats Mode

To see statistics about the XML file, run:

```sh
cargo run --release -- <path/to/your/file.xml>
```

## Building

To build the project from source, run:

```sh
cargo build --release
```

## Download Binaries

Pre-built binaries for Windows, macOS, and Linux are available on the [Releases page](https://github.com/ioma8/xmz/releases). Download the appropriate archive for your system, extract it, and run the `xmz` executable.

The binary is unsigned, so on macOS, you might need to run the following command to allow the system to run it:
```sh
xattr -r -d com.apple.quarantine xmz
```

---
<p align="center">Made with ❤️ in Rust</p>
