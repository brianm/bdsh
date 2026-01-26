---
yatl_version: 1
title: Add interactive features (attach to host, input)
id: qhd3p5cd
created: 2026-01-18T21:42:38.119626Z
updated: 2026-01-18T21:43:05.209954Z
author: Brian McCallister
priority: low
tags:
- polish
- ui
blocked_by:
- s7bsv7kh
- vyz6nsmn
---

From README: "the ability to go interactive if needed"

Interactive features to consider:
1. Attach to specific host's tmux window (for debugging, manual input)
2. Send input to all hosts simultaneously
3. Send input to specific host
4. Keybindings in client mode:
   - 'a' - attach to selected host window
   - 'k' - create .keep file
   - 'q' - quit
   - Arrow keys - navigate between hosts
   - Enter - expand/collapse diff for selected host

This leverages tmux's existing capabilities - we just need to provide a nice interface to access them.

---
# Log: 2026-01-18T21:42:38Z Brian McCallister

Created task.

---
# Log: 2026-01-18T21:43:05Z Brian McCallister

Added blocker: s7bsv7kh

---
# Log: 2026-01-18T21:43:05Z Brian McCallister

Added blocker: vyz6nsmn
