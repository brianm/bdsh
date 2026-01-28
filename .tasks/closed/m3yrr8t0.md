---
yatl_version: 1
title: 'Improve: Initial refresh error silently ignored'
id: m3yrr8t0
created: 2026-01-28T04:41:55.161633Z
updated: 2026-01-28T04:45:44.582793Z
author: Brian McCallister
priority: medium
tags:
- bug
- watch/mod.rs
---

In watch/mod.rs:419, the initial refresh result is silently discarded:

```rust
let _ = state.refresh(); // Initial refresh, ignore errors
```

**Problem**: If the initial refresh fails (e.g., permission denied on output directory), the user sees no error and the TUI just shows "No host directories found" forever.

**Location**: watch/mod.rs:419

**Options:**
1. Propagate the error: `state.refresh()?;`
2. Show error in TUI: store error state and display it
3. Log the error but continue: `if let Err(e) = state.refresh() { eprintln!("Warning: {}", e); }`

The current behavior masks real errors that the user should know about.

---
# Log: 2026-01-28T04:41:55Z Brian McCallister

Created task.

---
# Log: 2026-01-28T04:45:44Z Brian McCallister

Closed: Initial refresh error is now propagated instead of silently ignored
