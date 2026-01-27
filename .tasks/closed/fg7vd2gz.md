---
yatl_version: 1
title: Auto-detect content format (JSON/YAML/tabular)
id: fg7vd2gz
created: 2026-01-26T04:57:57.301111Z
updated: 2026-01-27T02:46:54.513289Z
author: Brian McCallister
priority: high
tags:
- cli
- hosts
blocked_by:
- 7a0a2txt
---

Detect format based on first non-whitespace character:
- `{` or `[` → JSON
- `---` → YAML  
- Otherwise → line-oriented tabular

```rust
enum Format { Json, Yaml, Tabular }

fn detect_format(content: &str) -> Format {
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        Format::Json
    } else if trimmed.starts_with("---") {
        Format::Yaml
    } else {
        Format::Tabular
    }
}
```

Route to appropriate parser based on detected format.

---
# Log: 2026-01-26T04:57:57Z Brian McCallister

Created task.

---
# Log: 2026-01-26T04:58:09Z Brian McCallister

Added blocker: 7a0a2txt

---
# Log: 2026-01-27T02:45:30Z Brian McCallister

Started working.

---
# Log: 2026-01-27T02:46:54Z Brian McCallister

Closed: Implemented format detection (JSON/YAML/Tabular). JSON and YAML return 'not implemented' errors for now - stubs for follow-up tasks. Added 6 tests.
