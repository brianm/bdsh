---
yatl_version: 1
title: 'Docs: Explain carriage return processing in clean_terminal_output'
id: k9h70avb
created: 2026-01-28T04:41:04.249078Z
updated: 2026-01-28T04:49:39.841011Z
author: Brian McCallister
priority: low
tags:
- docs
- consensus.rs
---

The `clean_terminal_output()` function in consensus.rs:651-687 handles carriage returns (`\r`) in a specific way, but the logic isn't well explained.

**Current behavior:**
- `\r` means "return to start of line, overwrite from there"
- If new text is shorter than existing, keeps the remainder
- Example: `"hello\rhi"` -> `"hillo"` (not `"hi"`)

This is correct terminal emulation, but the code comment just says:
```rust
// Process carriage returns: text after \r overwrites from start of line
```

**Suggestion**: Add examples in the comment:
```rust
/// Process carriage returns to simulate terminal behavior.
/// 
/// Examples:
/// - "hello\rhi" -> "hillo" (overwrite first 2 chars, keep rest)
/// - "hello\rworld" -> "world" (full overwrite)
/// - "a\rb\rc" -> "c" (multiple CRs, last wins)
```

**Location**: consensus.rs:651-687

---
# Log: 2026-01-28T04:41:04Z Brian McCallister

Created task.

---
# Log: 2026-01-28T04:49:39Z Brian McCallister

Closed: Added examples showing carriage return behavior
