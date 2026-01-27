---
yatl_version: 1
title: Implement .keep mechanism for preserving output
id: neyc99m7
created: 2026-01-18T21:42:06.807232Z
updated: 2026-01-27T19:27:02.931589528Z
author: Brian McCallister
priority: low
tags:
- polish
blocked_by:
- r5tbh396
---

From README: "By default, output directory is deleted on exit, but can be preserved with a flag. It will also look for a .keep which the client can drop in it, so you can decide to keep it interactively from within the client."

Implementation:
1. --keep flag creates .keep file at startup
2. Client mode can create .keep file interactively (keybinding?)
3. On exit, check for .keep before deleting output directory
4. If keeping, print the output directory path so user knows where to find it

This enables:
- Deciding to keep output after seeing something interesting
- Preserving output for debugging
- Post-mortem analysis of command execution

---
# Log: 2026-01-18T21:42:06Z Brian McCallister

Created task.

---
# Log: 2026-01-18T21:42:34Z Brian McCallister

Added blocker: r5tbh396

---
# Log: 2026-01-27T19:27:02Z Brian McCallister

Closed: Completed: --keep flag, 'K' keybinding in TUI toggles .keep marker, cleanup checks for .keep before deleting output dir, prints path when keeping
