---
yatl_version: 1
title: 'Bug: truncate() can panic on multi-byte UTF-8'
id: 6zy4rz6p
created: 2026-01-28T04:40:25.377526Z
updated: 2026-01-28T04:40:25.377526Z
author: Brian McCallister
priority: high
tags:
- bug
- consensus.rs
---

In `consensus.rs:563-569`, the `truncate()` function uses byte slicing which can panic on multi-byte UTF-8 characters:

```rust
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
```

**Problem**: `s.len()` returns bytes, and `&s[..n]` slices by bytes. If the cut point falls in the middle of a multi-byte character (e.g., emoji or non-ASCII), this panics.

**Location**: consensus.rs:563-569

**Fix**: Use `.chars()` iterator with `.take()` to properly handle character boundaries:
```rust
fn truncate(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}
```

---
# Log: 2026-01-28T04:40:25Z Brian McCallister

Created task.
