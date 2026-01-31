# bdsh

Better Distributed Shell - run commands on multiple hosts with a consensus view of the output.

![Demo](demo/cowsay.gif)

## Usage

```bash
bdsh host1,host2,host3 -- uname -a
bdsh @hosts.txt -- apt update
bdsh @"kubectl get nodes -o name" -- uptime
```

### Host Sources

- **Inline**: `host1,host2,host3`
- **File**: `@hosts.txt` - one host per line, supports tags and comments
- **Command**: `@"command"` - use output of command as host list

### Tag Filtering

Hosts file can include tags:

```
web1 :web :prod
web2 :web :prod
db1 :db :prod
dev1 :web :dev
```

Filter with:
- `:web` - hosts with tag
- `:web:prod` - hosts with both tags (AND)
- `:web,:db` - hosts with either tag (OR)

```bash
bdsh @hosts.txt :prod -- systemctl status nginx
```

## Architecture

bdsh creates a tmux session with:
- **Window 0**: Watch mode TUI showing consensus view of all output
- **Window 1-N**: One window per host running `ssh host command`

Output is captured to `$output_dir/$host/out.log` via `pipe-pane`.

## Watch Mode TUI

The consensus view shows output that's identical across hosts normally, and highlights differences:

```
[>2] Checking for upgrades...     <- 2 hosts differ, expandable
     [97] │ (0 candidates): 100%  <- 97 hosts have this
    host1 │ (1 candidates): 100%  <- 1 host differs
```

### Keys

| Key | Action |
|-----|--------|
| `↑↓` or `j/k` | Scroll |
| `→←` or `l/h` | Expand/collapse differences |
| `Tab` | Jump to next difference |
| `t` | Toggle tail mode (auto-scroll) |
| `e/c` | Expand/collapse all |
| `K` | Toggle keep output on exit |
| `q` | Quit |

### Status Indicators

- `⠋` (spinner) - running
- `⌨` (blinking) - waiting for input
- `✓` - success
- `✗` - failed

## Options

```
--watch <DIR>      Watch an output directory (standalone mode)
-o, --output-dir   Output directory (default: temp)
-k, --keep         Keep output directory on exit
--no-watch         Skip watch window, just run commands
```

## Development

### Building

```bash
cargo build           # Debug build
cargo build --release # Release build
cargo test            # Run tests
```

### Documentation

See [Additional Documentation](doc/bdsh-extra.md) for more details on configuration and advanced usage.

### Making a Release

Releases are automated via [cargo-dist](https://github.com/axodotdev/cargo-dist). When a version tag is pushed, GitHub Actions builds binaries for macOS and Linux, creates a GitHub release, and updates the Homebrew formula.

```bash
# Bump version, publish to crates.io, create tag
cargo release patch --execute --no-confirm  # or minor/major

# Push tag to trigger release workflow (using jj/git)
jj git push --all  # or: git push origin --tags
```

This will:
1. Publish to [crates.io](https://crates.io/crates/bdsh)
2. Build binaries for macOS (arm64, x86_64) and Linux (arm64, x86_64)
3. Create a GitHub release with downloadable archives (including man page)
4. Update the Homebrew formula at `brianm/homebrew-tools`

## Alternatives

- [dsh](https://www.netfort.gr.jp/~dancer/software/dsh.html.en)
- [pssh](https://code.google.com/archive/p/parallel-ssh/)
- [clusterssh](https://github.com/duncs/clusterssh)
- [pdsh](https://github.com/chaos/pdsh)
