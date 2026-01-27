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

The tool uses a tmux-based architecture with direct command execution:

**Session Mode**: Creates a tmux session with one window per host. Each window runs `ssh -t $host $command` with `pipe-pane` to capture output to `$output_dir/$host/out.log` while preserving full interactivity.

**Watch Mode** (planned): Will monitor the output directory for file changes, generate consensus views, and highlight differences between hosts.

### Key Files

- `src/main.rs` - Entry point, CLI handling, tmux session management
