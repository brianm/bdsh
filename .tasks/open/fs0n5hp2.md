---
yatl_version: 1
title: Convert watch mode to full TUI
id: fs0n5hp2
created: 2026-01-27T02:27:31.504314Z
updated: 2026-01-27T02:27:31.504314Z
author: Brian McCallister
priority: low
tags:
- ui
- future
---

The consensus view currently just prints and refreshes. Convert to a proper TUI with:

- Scrollable output
- Keyboard navigation between hosts
- Expand/collapse diff sections
- Maybe split panes for side-by-side comparison
- Search within output

Consider ratatui or similar TUI framework.

Not needed for MVP - current print-and-refresh works for now.

---
# Log: 2026-01-27T02:27:31Z Brian McCallister

Created task.
