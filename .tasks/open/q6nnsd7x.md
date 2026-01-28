---
yatl_version: 1
title: 'Refactor: Remove unused ''similar'' crate dependency'
id: q6nnsd7x
created: 2026-01-28T04:40:42.378518Z
updated: 2026-01-28T04:40:42.378518Z
author: Brian McCallister
priority: low
tags:
- cleanup
- dependencies
---

The `similar` crate (version 2.7.0) is listed in Cargo.toml but is not currently used in the codebase.

**Location**: Cargo.toml:18

There's a comment in watch/mod.rs:23 noting it's available:
```rust
// Note: similar crate still available if we need diff-based view later
```

**Options:**
1. Remove the dependency if diff-based view isn't planned soon
2. Keep it if the feature is coming, but add a TODO comment in Cargo.toml explaining why

Unused dependencies increase build time and binary size unnecessarily.

---
# Log: 2026-01-28T04:40:42Z Brian McCallister

Created task.
