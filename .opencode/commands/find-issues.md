---
description: Analyze codebase, validate findings, and create max $ARGUMENTS GitHub issues
agent: orchestrator
subtask: false
---

@orchestrator

You are being invoked to analyze the codebase and create at most $ARGUMENTS GitHub issues. Execute the find-issues workflow by delegating to subagents according to the rules.

## Reference Rules
Load these rules for this operation:
- @.opencode/rules/validation.rules - Validation criteria and thresholds
- @.opencode/rules/issue-processing.rules - Input parsing, output format, deduplication
- @.opencode/rules/github-operations.rules - GitHub issue creation

## Flags
- **$ARGUMENTS**: Maximum GitHub issues to create (default: 10)
- If agents find fewer issues than $ARGUMENTS, create all found issues
- If agents find more issues than $ARGUMENTS, MERGE similar findings

## Subagents to Invoke (Parallel)

Launch these 6 specialized agents in parallel with unique session_ids:

| Session ID | Agent | Focus Area |
|------------|-------|------------|
| **security-1** | @security-1 | Auth bypass, injection, crypto failures, secrets, input validation |
| **critical-1** | @critical-1 | Panics, unwrap on None/Error, OOB access, division by zero |
| **performance-1** | @performance-1 | Memory leaks, allocations, slow algos, missing caches |
| **reliability-1** | @reliability-1 | Missing error handling, poor errors, missing retries |
| **testing-1** | @testing-1 | Missing tests, low coverage, untested edge cases |
| **tech-debt-1** | @tech-debt-1 | Code duplication, dead code, complex functions |

## Orchestration Steps

1. **Launch All 6 Agents Simultaneously**
   - Each outputs to `docs/outputs/issues/{category}/{filename}.md`
   - Wait for ALL to complete before proceeding

2. **Deduplicate Across Agents**
   - Remove findings with same title OR same file + similar problem
   - Keep first occurrence

3. **Merge Into Max $ARGUMENTS Issues**
   - If findings > $ARGUMENTS, group by: same file → same category → same risk → related functionality
   - Apply merged template from rules

4. **Validator Loop (Max 3 Rounds)**

   **Round 1: Content Validation**
   - Invoke @validator-content-1 (session: validator-content-1)
   - Output: docs/outputs/validation/round1_content_review.yaml
   - Threshold: 7/10

   **Round 2: Structure Validation**
   - Invoke @validator-structure-1 (session: validator-structure-1)
   - Output: docs/outputs/validation/round2_structure_review.yaml
   - Threshold: 8/10

   **Round 3: Integration Validation**
   - Invoke @validator-integration-1 (session: validator-integration-1)
   - Output: docs/outputs/validation/round3_integration_review.yaml
   - Verify improvements, check for regressions

   **Validation Decision:**
   - All findings must pass integration (can_create_issue: true)
   - If any finding fails, improve it and re-run validation (max 3 rounds)
   - After 3 rounds, create issues for passed findings only

5. **Create GitHub Issues**
   - For each validated finding, use improved versions from validation
   - Title: `[P{1-3}] {title}`
   - Body: problem, impact, reproduction, AC, context
   - Labels: All validated labels
   - Use `gh issue create`

6. **Report Summary**
   - Issues created: N (max $ARGUMENTS)
   - Findings found: M
   - Findings merged: M - N
   - Validation rounds: K
   - Issues failed validation: F
   - Repository URL

## Output Format Per Finding

Each agent outputs to `docs/outputs/issues/{category}/{filename}.md`:

```yaml
---
title: [P1] Clear title under 70 characters
problem: One sentence describing the issue
impact: One sentence on user/system impact
reproduction: Steps to reproduce or observe
acceptance_criteria:
- Criterion 1
- Criterion 2
effort: 1-5 scale
risk: High|Medium|Low
category: security|critical|performance|reliability|testing|tech-debt
files:
- src/file.rs
labels: security|crash|performance|error-handling|testing|debt
---

Additional context...
```

## Merged Issue Template

When merging findings:
```yaml
---
title: [P1] Consolidated: {primary_title}
merged_from: "{count} findings merged"
problems:
- {problem 1}
- {problem 2}
impact: Combined impact
reproduction: |
  Finding 1: {reproduction 1}
  Finding 2: {reproduction 2}
acceptance_criteria:
- Combined criterion
effort: {highest effort}
risk: {highest risk}
category: {primary category}
files:
- {all unique files}
labels: {all unique labels}
source_files:
- docs/outputs/issues/{category}/{file1}.md
- docs/outputs/issues/{category}/{file2}.md
---
```

## Constraints (Critical)
- Maximum GitHub issues: $ARGUMENTS
- Validation max rounds: 3
- Content threshold: 7/10
- Structure threshold: 8/10
- If findings <= $ARGUMENTS: create separately (no merge)
- If findings > $ARGUMENTS: merge into $ARGUMENTS issues
- Deduplicate across agents first
- Use YAML for structured validation feedback
- If no files in `docs/outputs/issues/`: report "No issues found" and exit

Begin analysis. Launch 6 agents → deduplicate → merge if needed → validate (max 3 rounds) → create max $ARGUMENTS issues.
