---
description: Resolve a GitHub issue autonomously - fix, test, create PR
agent: general
---

Resolve GitHub issue #$ARGUMENTS.

**Steps:**
1. `git worktree add ../issue$ARGUMENTS-worktree main --detach && cd ../issue$ARGUMENTS-worktree`
2. Read issue context: `gh issue view $ARGUMENTS --json title,body`
3. Fix the issue (delegate to subagents as needed)
4. Run tests: `cargo test -p arbitrage-core`
5. Commit: `git add -A && git commit -m "feat: resolve issue #$ARGUMENTS"`
6. Push and create PR: `git push origin HEAD && gh pr create --title "Fix issue #$ARGUMENTS" --body "Resolved" --label "auto-pr,needs-review"`
7. Wait for user `/approve` before merging

Report progress at each step.
