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

## Project Overview

bdsh (Better Distributed Shell) is a tool for running commands on multiple hosts simultaneously via SSH, with a consensus view of output and interactive capabilities. Status: early development.

## Architecture

The tool uses a two-mode tmux-based architecture:

**Server Mode (default)**: Creates a tmux control session (`-C new-session`) that spawns windows for each host. Commands run as `ssh $host $command | tee $output_dir/$host/out.log` to capture output while preserving interactivity.

**Client/Watch Mode**: Monitors the output directory for file changes, generates consensus views, and highlights differences between hosts. Operates independently of tmux.

### Key Files

- `src/main.rs` - Entry point, CLI handling, `Job` struct defining work units (root dir, host, command)
- `src/tmux.rs` - Tmux control mode interface:
  - `Control` struct - Manages tmux process with stdin/stdout pipes
  - `Window` struct - Represents tmux windows
  - `Notification` enum - Parses tmux control mode protocol (`%session-changed`, `%begin`, `%end`, `@` output)
  - `TmuxError` - Custom error types via thiserror

### Tmux Control Protocol

The `Control` struct communicates with tmux via control mode, which uses line-based notifications prefixed with `%` or `@`. Key patterns:
- Uses `-P -F` flags to extract window IDs when creating windows
- Tracks state via file system rather than notification tracking for debuggability
