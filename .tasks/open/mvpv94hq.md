---
yatl_version: 1
title: 'Refactor: Split consensus.rs into computation and rendering'
id: mvpv94hq
created: 2026-01-28T04:41:22.522680Z
updated: 2026-01-28T04:41:22.522680Z
author: Brian McCallister
priority: low
tags:
- refactor
- organization
---

consensus.rs is 740 lines and handles multiple concerns:

1. **Data structures** (ConsensusLine, Selection, ConsensusView) - Lines 1-63
2. **View state management** (scroll, expand, collapse) - Lines 65-299
3. **Scroll offset calculation** - Lines 301-344
4. **Display line building** - Lines 346-559
5. **Consensus computation** - Lines 571-649
6. **Terminal output cleaning** - Lines 651-715
7. **Widget rendering** - Lines 717-740

**Suggestion**: Split into:
- `consensus/types.rs` - ConsensusLine, Selection structs
- `consensus/compute.rs` - compute_consensus, make_differs, clean_terminal_output
- `consensus/view.rs` - ConsensusView state management and navigation
- `consensus/widget.rs` - ConsensusViewWidget and build_display_lines

This would make each file ~150-200 lines and single-purpose.

**Location**: src/watch/consensus.rs

---
# Log: 2026-01-28T04:41:22Z Brian McCallister

Created task.
