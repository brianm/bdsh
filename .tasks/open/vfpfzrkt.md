---
yatl_version: 1
title: Use dedicated tmux socket for isolation
id: vfpfzrkt
created: 2026-01-18T21:40:26.291986Z
updated: 2026-01-18T21:40:36.008977Z
author: Brian McCallister
priority: medium
tags:
- foundation
- tmux
---

Use tmux -L socket_name or -S socket_path to avoid polluting user's default tmux server.

Store socket in the output directory so it's cleaned up automatically. Update Control::start_session() to accept socket path parameter.

Benefits:
- Won't interfere with user's existing tmux sessions
- Clean separation per bdsh invocation
- Socket removed when output dir cleaned up

---
# Log: 2026-01-18T21:40:26Z Brian McCallister

Created task.
