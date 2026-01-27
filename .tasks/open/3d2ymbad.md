---
yatl_version: 1
title: Parse JSON host data with json-pointer
id: 3d2ymbad
created: 2026-01-26T04:58:23.356278Z
updated: 2026-01-26T04:58:36.836Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
blocked_by:
- fg7vd2gz
---

Parse JSON arrays with optional json-pointer for hostname:

- If array of strings: `["h1", "h2"]` → use strings directly
- If array of objects: use `--host-ptr` to locate hostname
  - `--host-ptr /hostname` for `[{"hostname": "h1", "role": "web"}]`

Add `serde_json` and `jsonptr` crates to Cargo.toml.

```rust
fn parse_json(content: &str, host_ptr: Option<&str>) -> Result<Vec<Host>>
```

Keep full JSON object as metadata for filtering.

---
# Log: 2026-01-26T04:58:23Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:58:36Z Brian McCallister

Added blocker: fg7vd2gz
