# xmz

![CI](https://github.com/USERNAME/REPOSITORY/actions/workflows/ci.yml/badge.svg)

A high-performance, zero-allocation XML parser with a TUI for interactive traversal.

## Features

- **Blazing Fast:** Parses large XML files at high speed by leveraging `memmap` and `memchr`.
- **Zero-Allocation Parsing:** The core parser operates without memory allocations for maximum efficiency.
- **Interactive TUI:** Navigate XML trees interactively with a user-friendly terminal interface.
- **Statistics Mode:** Get insights into your XML structure, including tag counts and max depth.

## Usage

### TUI Mode

To explore an XML file interactively, run:

```sh
cargo run --release -- --tui
```

### Stats Mode

To see statistics about the XML file, run:

```sh
cargo run --release
```

## Building

To build the project, run:

```sh
cargo build --release
```
