---
yatl_version: 1
title: 'Docs: Add high-level doc comment for ConsensusView selection model'
id: sda2ycbz
created: 2026-01-28T04:41:04.182491Z
updated: 2026-01-28T04:49:10.846398Z
author: Brian McCallister
priority: medium
tags:
- docs
- consensus.rs
---

The `ConsensusView` component has a complex two-level selection model that isn't documented:

1. **Level 1**: Line index (selecting consensus lines)
2. **Level 2**: Variant index within an expanded Differs line

The `Selection` struct captures this:
```rust
pub(crate) struct Selection {
    pub(crate) line_index: usize,
    pub(crate) variant_index: Option<usize>,
}
```

**Navigation behavior is complex:**
- Down arrow on collapsed Differs -> move to next line
- Down arrow on expanded Differs -> enter variants (variant_index = Some(0))
- Down arrow on last variant -> exit to next line
- Up arrow reverses this

**Location**: consensus.rs:36-56 and scroll_up/scroll_down methods

**Suggestion**: Add a module-level doc comment explaining:
1. The two-level selection model
2. How navigation works with expanded/collapsed states
3. Why this design was chosen (hierarchical diff viewing)

---
# Log: 2026-01-28T04:41:04Z Brian McCallister

Created task.

---
# Log: 2026-01-28T04:49:10Z Brian McCallister

Closed: Added comprehensive doc comments explaining the two-level selection model
