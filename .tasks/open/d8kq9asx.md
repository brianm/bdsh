---
yatl_version: 1
title: Parse tabular host data with column selection
id: d8kq9asx
created: 2026-01-26T04:58:12.822491Z
updated: 2026-01-26T04:58:19.661303Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
blocked_by:
- fg7vd2gz
---

Parse line-oriented tabular data:

1. If line contains tabs → split on tab
2. Otherwise → split on whitespace (\s+)
3. Use `--host-col` (1-indexed) to select hostname column
4. Store remaining columns as metadata for filtering

```rust
struct Host {
    hostname: String,
    columns: Vec<String>,  // all columns for filtering
}

fn parse_tabular(content: &str, host_col: usize) -> Vec<Host>
```

Handle edge cases: empty lines, comments (#)?

---
# Log: 2026-01-26T04:58:12Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:58:19Z Brian McCallister

Added blocker: fg7vd2gz
