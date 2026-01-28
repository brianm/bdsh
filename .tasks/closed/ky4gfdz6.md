---
yatl_version: 1
title: 'Improve: Derive Default for Selection and ConsensusView'
id: ky4gfdz6
created: 2026-01-28T04:41:55.089396Z
updated: 2026-01-28T04:51:06.783992Z
author: Brian McCallister
priority: low
tags:
- cleanup
- consensus.rs
---

`Selection::new()` and `ConsensusView::new()` implement manual constructors that just set default values:

```rust
impl Selection {
    pub(crate) fn new() -> Self {
        Self {
            line_index: 0,
            variant_index: None,
        }
    }
}

impl ConsensusView {
    pub(crate) fn new() -> Self {
        Self {
            consensus: Vec::new(),
            selection: Selection::new(),
            has_hosts: false,
        }
    }
}
```

**Suggestion**: Derive `Default` instead:
```rust
#[derive(Clone, Debug, Default)]
pub(crate) struct Selection {
    pub(crate) line_index: usize,  // defaults to 0
    pub(crate) variant_index: Option<usize>,  // defaults to None
}
```

This is more idiomatic Rust and reduces boilerplate.

**Location**: consensus.rs:44-56, 66-72

---
# Log: 2026-01-28T04:41:55Z Brian McCallister

Created task.

---
# Log: 2026-01-28T04:51:06Z Brian McCallister

Closed: Derived Default for Selection and ConsensusView, simplified new() methods
