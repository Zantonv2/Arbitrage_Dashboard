---
description: Find issues in codebase and generate GitHub issues (limits to $ARGUMENTS issues max)
agent: build
subtask: false
---

You are an autonomous Engineering Insight Orchestrator.

**TASK:**
1. Launch 5 parallel subagents to analyze the codebase:
   - **Critical subagent**: Finds crashes, security issues, data corruption, production outages
   - **Performance subagent**: Finds latency, memory, CPU, scalability bottlenecks
   - **Testing subagent**: Finds missing tests, coverage gaps, flaky tests
   - **Maintainability subagent**: Finds complexity, duplication, technical debt
   - **Robustness subagent**: Finds error handling gaps, retry logic issues, observability gaps

2. Each subagent must output findings to `docs/outputs/issues/{category}/{filename}.md`
   Each file follows this strict template:
   ```markdown
   title: [P1] Clear title under 70 chars
   problem: One sentence describing the issue
   impact: One sentence on user/system impact
   reproduction: Steps to reproduce or observe
   acceptance_criteria:
   - Criterion 1
   - Criterion 2
   effort: 2
   risk: Medium
   files:
   - src/file.rs
   labels: bug,security
   ```

3. After all subagents complete, read ALL .md files from `docs/outputs/issues/`

4. **LIMIT TO $ARGUMENTS ISSUES MAXIMUM**:
   - If more than $ARGUMENTS files exist, prioritize by:
     a. Risk level (High > Medium > Low)
     b. Within same risk, by effort (lower is better)
     c. Keep exactly $ARGUMENTS issues
   - Output ONLY the top $ARGUMENTS issues

5. Generate GitHub issues from the selected files:
   - Title: `[P1] {title}`
   - Body: Include problem, impact, reproduction, acceptance criteria, meta
   - Labels: Apply all labels from the file
   - Use `gh issue create` for each issue

6. Report:
   - Issues created: N
   - Issues skipped: M (because of limit)
   - Repository URL for issues

**CONSTRAINTS:**
- One finding = One file = One GitHub issue
- Never modify subagent findings
- Never merge multiple findings into one issue
- Output issues separated by exactly two newlines

**If no files exist in docs/outputs/issues/:**
- Report "No issues found" and exit

Begin now. Launch all 5 subagents simultaneously, wait for completion, then generate issues.
