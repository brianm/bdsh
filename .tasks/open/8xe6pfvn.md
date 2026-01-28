---
yatl_version: 1
title: 'Refactor: TagFilter::All vs empty Groups is redundant'
id: 8xe6pfvn
created: 2026-01-28T04:41:55.011367Z
updated: 2026-01-28T04:41:55.011367Z
author: Brian McCallister
priority: low
tags:
- refactor
- hosts.rs
---

The `TagFilter` enum has two ways to express "match all hosts":

```rust
pub enum TagFilter {
    All,
    Groups(Vec<Vec<String>>),  // Empty vec also matches all
}
```

In `parse_tag_filter()` (hosts.rs:205-228):
```rust
if groups.is_empty() {
    Ok(TagFilter::All)
} else {
    Ok(TagFilter::Groups(groups))
}
```

And in `matches_filter()` (hosts.rs:231-241):
```rust
match filter {
    TagFilter::All => true,
    TagFilter::Groups(groups) => { ... }
}
```

**Problem**: `TagFilter::Groups(vec![])` would also match all hosts (empty .any() is false, but this is never created). The code manually converts empty groups to `All`, but the type system allows the redundant state.

**Options:**
1. Keep as-is (simple, works)
2. Use `NonEmpty<Vec<String>>` to make empty groups unrepresentable
3. Document why `All` exists vs empty Groups

This is minor but represents a potential source of confusion.

**Location**: hosts.rs:16-23, 205-228

---
# Log: 2026-01-28T04:41:55Z Brian McCallister

Created task.
