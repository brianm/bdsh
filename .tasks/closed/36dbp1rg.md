---
yatl_version: 1
title: 'Refactor: Status should be an enum, not String'
id: 36dbp1rg
created: 2026-01-28T04:40:42.314232Z
updated: 2026-01-28T04:47:18.889971Z
author: Brian McCallister
priority: medium
tags:
- refactor
- type-safety
---

Status values ("running", "success", "failed", "pending") are passed around as Strings throughout the codebase:

- **main.rs**: Writes status as string literals
- **watch/mod.rs:543-548**: Reads status, returns String
- **watch/status_bar.rs:46-51**: Matches on string slices

**Problem**: No compile-time guarantees about valid status values. Easy to introduce typos.

**Suggestion**: Create a Status enum:
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Pending,
    Running,
    Success,
    Failed,
}

impl Status {
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "running" => Self::Running,
            "success" => Self::Success,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
    
    pub fn as_str(&self) -> &'static str { ... }
}
```

**Files affected**: main.rs, watch/mod.rs, watch/status_bar.rs

---
# Log: 2026-01-28T04:40:42Z Brian McCallister

Created task.

---
# Log: 2026-01-28T04:47:18Z Brian McCallister

Closed: Added Status enum with from_str/as_str methods, updated all usages
