---
description: Autonomously resolve GitHub issue(s) - extract tasks, delegate to subagents, validate, and create ONE PR with optional auto-merge
agent: orchestrator
subtask: false
---

@orchestrator

You are being invoked to resolve GitHub issue(s) $ARGUMENTS. Execute the fix-issue workflow by delegating to subagents according to the rules.

## Reference Rules
Load these rules for this operation:
- @.opencode/rules/git-branching.rules - Branch naming, worktree isolation, commit messages
- @.opencode/rules/validation.rules - Validation criteria, thresholds, reconciliation
- @.opencode/rules/issue-processing.rules - Input parsing, output format, batch processing
- @.opencode/rules/github-operations.rules - GitHub API operations, PR creation, artifacts

## Quick Reference
- Issue numbers: $ARGUMENTS (single: #123, range: 105-110, list: 105 106 107)
- Repository: Current repo (auto-detected via `gh repo view`)
- Worktree path: `../issue{ISSUENUM}-worktree` (per-issue)
- Remote: `upstream` points to GitHub repository
- Source branch: `upstream/main` on GitHub (permanent source, syncs with local upstream)
- Target branch: `main` on GitHub (PRs merge here)
- Feature branch: `batch-fix-{ISSUES}` (temporary, deleted after merge)

## Flags
- `--auto`: Enable strict auto-merge (5 validators, 95% avg success, ONE PR only)
- Default: Manual approval required (2 validators, ONE PR only)

## Orchestration Steps

1. **Parse & Validate Input**
   - Parse $ARGUMENTS for issue specification (single, range, list, comma-separated)
   - Validate each extracted number (positive integer > 0)
   - Deduplicate issues
   - Pre-flight check: gh api for each issue, skip closed/non-existent
   - Create batch state file: output/issue-fix-artifacts/batch_state.json
   - Calculate dynamic max_reconciliations (base 3 + batch_size + risk + mode factors)

2. **Batch Preparation**
   - Check for uncommitted changes
   - Remove existing issue worktrees
   - Create batch worktree: `../batch-fix-worktree`
   - Sync baseline: git fetch upstream && git reset --hard upstream/main
   - Record environment info

3. **Per-Issue Processing Loop**
   For each issue:
   - Create isolated worktree for this issue
   - Capture issue context (comments, timeline)
   - Extract tasks and cluster them (task_extraction.json, clusters.json)
   - Launch subagents per cluster with unique session_ids
   - Each subagent commits with `[AUTO] Issue $ISSUE - Cluster {id}: {summary}`
   - Validate with 5 validators (AUTO_MODE) or 2 validators (default)
   - Use @fix-issue-validator subagent for validation
   - If fails after reconciliations: log to failed_issues.json, continue to next issue
   - Cherry-pick commits to batch worktree

4. **Final Validation**
   - cargo check --all-features
   - cargo test --all
   - cargo clippy --all-features-threads=1 -D warnings
   - cargo fmt -- --check
   - Verify all acceptance criteria

5. **One PR Creation**
   - Create feature branch: batch-fix-{ISSUES}
   - Remove output/ artifacts
   - Commit with structured message
   - Push to upstream
   - Create ONE PR with validation summary

6. **Merge Decision**
   - AUTO_MODE with 95%+: auto-merge, close issues, sync upstream branch
   - Default: wait for /approve, then merge, close issues, sync upstream

7. **Finalization**
   - Generate batch_summary.md
   - Cleanup worktrees and branches
   - Stash pop if stashed changes existed

## Subagents to Invoke

- **For cluster implementation**: Task tool with cluster-specific sessions
- **For validation**: @fix-issue-validator with unique session_ids (validator-1 through validator-5)
- **For task extraction**: Use explore/read tools to analyze issue context

## Constraints (Critical)
- Worktree isolation: Per-issue worktrees + ONE batch worktree
- Sequential issue processing, parallel cluster processing
- NO partial PRs - ONE PR only when ALL issues 100% complete
- If ANY issue fails, batch FAILS - no partial PRs
- AUTO_MODE: 95% average, NO LAZINESS, all validators must pass
- Default mode: 2 validators, >=7/10 on EACH criterion
- User approval mandatory before merge (except AUTO_MODE with 95%+)
- Remove output/ folder before PR commit
- UPSTREAM BRANCH SYNC: After PR merge, sync GitHub upstream to match merged commit
- Dynamic reconciliations: calculated per batch, adaptive extension (+1 if improvement >=10%, -1 if regression >5%)
- TRANSIENT FAILURES: Retry up to 3 times with 5s backoff for network errors, cargo lock, timeouts

## Output Artifacts
All artifacts go to `output/issue-fix-artifacts/`:
- environment.json
- batch_state.json
- skipped_issues.json
- failed_issues.json
- validation_progress.json
- per-issue/issue_{N}/*
- batch_failure_report.json (if batch fails)
- batch_summary.md (on success)

Execute the fix-issue workflow now.
