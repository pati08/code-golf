# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo test           # Run tests
cargo check          # Type-check without building
docker build .       # Build with docker
```

## Toolchain & Build Setup

- **Rust nightly** is required (`rust-toolchain.toml` pins to `channel = "nightly"`)
- **Cranelift** codegen backend is used for dev builds (faster compilation); dependencies use LLVM
- **mold** linker with clang is configured in `.cargo/config.toml` for fast linking
- Edition 2024
- Docker - this is the primary intended build system. Includes:
  - docker-compose.yml with dev facilities
  - Dockerfile with cargo-chef and runtime dependencies

## Project Purpose

This is a code golf competition platform — a web service/tool that hosts and judges code golf challenges, tracks submissions by byte count, and manages competitions.

## Project Architecture

### File Structure

Source tree:
src
├── admin
│   ├── handlers.rs
│   └── mod.rs
├── app.rs
├── auth
│   ├── handlers.rs
│   └── mod.rs
├── config.rs
├── db
│   ├── mod.rs
│   └── models.rs
├── error.rs
├── feedback
│   ├── handlers.rs
│   └── mod.rs
├── main.rs
├── problems
│   ├── handlers.rs
│   └── mod.rs
├── profile
│   └── mod.rs
├── runner
│   ├── languages.rs
│   ├── mod.rs
│   └── sandbox.rs
├── scoreboard
│   ├── handlers.rs
│   └── mod.rs
├── scoring.rs
└── submissions
    ├── handlers.rs
    ├── judge.rs
    └── mod.rs
