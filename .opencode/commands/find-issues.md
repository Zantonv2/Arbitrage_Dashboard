---
description: Analyze codebase and create max $ARGUMENTS GitHub issues (merges similar findings)
agent: build
subtask: false
---

You are an Engineering Insight Orchestrator. Analyze codebase, find issues, and consolidate them into at most $ARGUMENTS GitHub issues.

**FLAGS:**
- **$ARGUMENTS**: Maximum number of GitHub issues to create (default: 10)
- If agents find fewer issues than $ARGUMENTS, create all found issues
- If agents find more issues than $ARGUMENTS, MERGE similar findings

**WORKFLOW:**

1. ANALYZE WITH SUBAGENTS
   Launch 5 parallel subagents with unique session_ids:
   - **Critical** (session: critical-1): Crashes, security, data corruption, outages
   - **Performance** (session: perf-1): Latency, memory, CPU, scalability
   - **Testing** (session: test-1): Missing tests, coverage gaps, flaky tests
   - **Maintainability** (session: maint-1): Complexity, duplication, technical debt
   - **Robustness** (session: robust-1): Error handling, retries, observability, edge cases

   Each subagent outputs findings to `docs/outputs/issues/{category}/{filename}.md`

2. FINDING FORMAT
   Each finding must use YAML frontmatter:

   ```
   ---
   title: [P1] Clear title under 70 characters
   problem: One sentence describing the issue
   impact: One sentence on user/system impact
   reproduction: Steps to reproduce or observe
   acceptance_criteria:
   - Criterion 1
   - Criterion 2
   effort: 1-5 scale (1=low, 5=high)
   risk: High|Medium|Low
   category: critical|performance|testing|maintainability|robustness
   files:
   - src/file.rs
   labels: bug|security|performance|testing|maintenance|robustness
   ---
   ```

3. DEDUPLICATE ACROSS SUBAGENTS
   Before merging, remove duplicate findings:
   - Same title = duplicate
   - Same file + similar problem = duplicate
   - Keep first occurrence, mark others as duplicate

4. MERGE INTO MAX $ARGUMENTS ISSUES
   If findings > $ARGUMENTS, group and consolidate:

   **Grouping priority:**
   a. **Same file/module** - group issues affecting the same code
   b. **Same category** - group security with security, performance with performance
   c. **Same risk level** - group High with High
   d. **Related functionality** - group related features

   **Merging template:**
   ```
   ---
   title: [P1] Consolidated: {primary_title}
   merged_from: {count} findings merged
   problems:
   - {problem 1}
   - {problem 2}
   - {problem N}
   impact: Combined impact on users and system
   reproduction: |
     Finding 1: {reproduction 1}
     Finding 2: {reproduction 2}
   acceptance_criteria:
   - Combined criterion 1
   - Combined criterion 2
   effort: highest effort among merged
   risk: highest risk among merged
   category: {primary category}
   files:
   - {all unique files}
   labels: {all unique labels}
   source_files:
   - docs/outputs/issues/{category}/{original_file1}.md
   - docs/outputs/issues/{category}/{original_file2}.md
   ---
   ```

   **Algorithm:**
   - Sort by risk (High > Medium > Low), then effort (low > high)
   - Create $ARGUMENTS groups
   - Put remaining findings into the last group if needed
   - If $ARGUMENTS = 1, merge ALL into single issue

5. CREATE GITHUB ISSUES
   For each consolidated finding:
   - Title: `[P{1-3}] {title}` (P1=High, P2=Medium, P3=Low)
   - Body: Include all merged problems, impacts, reproductions, acceptance criteria
   - Labels: Apply all labels from merged findings
   - Use `gh issue create`

6. REPORT
   - Issues created: N (max $ARGUMENTS)
   - Findings found: M
   - Findings merged: M - N
   - Repository URL for all issues

**CONSTRAINTS:**
- Maximum issues to create: $ARGUMENTS
- If findings <= $ARGUMENTS: create each as separate issue (no merging)
- If findings > $ARGUMENTS: MUST merge into exactly $ARGUMENTS issues
- Never modify problem statements when merging
- Deduplicate across subagents first
- If no files in `docs/outputs/issues/`: report "No issues found" and exit

Begin analysis. Launch all 5 subagents simultaneously, deduplicate, merge if needed, create max $ARGUMENTS GitHub issues.
