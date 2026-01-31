# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build           # Debug build
cargo build --release # Release build
cargo test            # Run all tests
cargo run             # Run the binary
```

## Development Workflow

- Use `jj` (Jujutsu) for version control, not git
- Use `/yatl` for planning and tracking work
- Do not check in `.claude/` directory

## Man Page

The man page is generated at build time from `src/cli.rs` and `doc/bdsh-extra.md`. Find it at `target/*/build/bdsh-*/out/bdsh.1` after building.

## Project Overview

bdsh (Better Distributed Shell) is a tool for running commands on multiple hosts simultaneously via SSH, with a consensus view of output and interactive capabilities. Status: early development.

## Architecture

The tool uses a tmux-based architecture with direct command execution:

**Session Mode**: Creates a tmux session with one window per host. Each window runs `ssh -t $host $command` with `pipe-pane` to capture output to `$output_dir/$host/out.log` while preserving full interactivity.

**Watch Mode** (planned): Will monitor the output directory for file changes, generate consensus views, and highlight differences between hosts.

### Key Files

- `src/main.rs` - Entry point, CLI handling, tmux session management

## Releasing

```bash
# Make a release (bumps version, tags, pushes to crates.io)
cargo release patch --execute  # or minor/major

# Tag push triggers GitHub Actions which:
# 1. Builds binaries for 4 platforms
# 2. Creates GitHub Release with tarballs
# 3. Updates Homebrew tap
# 4. Updates AUR package
```

### Local Build/Test

```bash
# Build release tarball for current platform
make dist

# Generate packaging files (after tarballs exist in dist/out/)
make formula    # Homebrew formula
make pkgbuild   # AUR PKGBUILD

# Test AUR package on Arch Linux
cd dist/out && makepkg -si
```
