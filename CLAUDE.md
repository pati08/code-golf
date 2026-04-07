# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build          # Build (uses cranelift backend for fast dev builds)
cargo run            # Build and run
cargo test           # Run tests
cargo check          # Type-check without building
```

## Toolchain & Build Setup

- **Rust nightly** is required (`rust-toolchain.toml` pins to `channel = "nightly"`)
- **Cranelift** codegen backend is used for dev builds (faster compilation); dependencies use LLVM
- **mold** linker with clang is configured in `.cargo/config.toml` for fast linking
- Edition 2024

## Project Purpose

This is a code golf competition platform — a web service/tool that hosts and judges code golf challenges, tracks submissions by byte count, and manages competitions.
