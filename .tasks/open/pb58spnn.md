---
yatl_version: 1
title: 'Test: TestSshd uses ''which'' command that may not exist'
id: pb58spnn
created: 2026-01-28T04:41:39.061548Z
updated: 2026-01-28T04:41:39.061548Z
author: Brian McCallister
priority: low
tags:
- test
- portability
---

In tests/common/mod.rs:23-31, the test SSH server uses `which sshd` to find sshd:

```rust
if Command::new("which")
    .arg("sshd")
    .output()
    .map(|o| !o.status.success())
    .unwrap_or(true)
{
    eprintln!("sshd not found, skipping SSH tests");
    return None;
}
```

**Problem**: `which` is not available on all systems (some minimal containers, Windows with WSL, etc.)

**Location**: tests/common/mod.rs:23-31 and 34-40

**Options:**
1. Try running `sshd --version` directly to check availability
2. Use `std::process::Command::new("sshd")` with proper error handling
3. Use the `which` crate for cross-platform lookup

This is a test-only issue but could cause confusion when tests skip unexpectedly.

---
# Log: 2026-01-28T04:41:39Z Brian McCallister

Created task.
