---
yatl_version: 1
title: 'Improve: HOME fallback to ''.'' is surprising behavior'
id: jzh8b9rv
created: 2026-01-28T04:41:38.988930Z
updated: 2026-01-28T04:41:38.988930Z
author: Brian McCallister
priority: low
tags:
- ux
- hosts.rs
---

In hosts.rs:113, when `HOME` env var is not set, it defaults to ".":

```rust
let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
```

This means the config path becomes `./.config/bdsh/hosts` instead of failing with a clear error.

**Problem**: 
- User expects `~/.config/bdsh/hosts`
- Gets `./.config/bdsh/hosts` silently
- Confusing when config "doesn't work"

**Location**: hosts.rs:113

**Options:**
1. Return an error when HOME is not set
2. Use `dirs` crate for proper XDG handling
3. At minimum, log a warning

This is an edge case (HOME is almost always set) but the current behavior is surprising.

---
# Log: 2026-01-28T04:41:38Z Brian McCallister

Created task.
