---
yatl_version: 1
title: Fix man page version to match release version
id: t1sa6sdj
created: 2026-01-29T18:44:41.480980026Z
updated: 2026-01-29T18:44:41.480980026Z
author: Brian McCallister
priority: medium
---

The man page shows the previous version because it's generated before version bumps.

Currently:
1. Version is bumped in Cargo.toml
2. Release tag is created  
3. Man page still has old version from before the bump

Need to:
- Regenerate man page after version bump but before release tag
- Or find a way to generate/update the version in the man page as part of the release process
- Possibly add to cargo-release hooks or create a script that does: bump version → regenerate man page → commit both → tag

The version appears in two places in the man page:
- .TH bdsh 1 "bdsh X.Y.Z"
- .SH VERSION section

Related files:
- tests/generate_man.rs - regeneration script
- doc/bdsh.1 - checked-in man page
- Cargo.toml - source of version

---
# Log: 2026-01-29T18:44:41Z Brian McCallister

Created task.
