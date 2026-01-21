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

**AGENTS:**

Launch these 6 specialized agents in parallel, each with a unique session_id:

| Session ID | Focus Area | What to Find |
|------------|------------|--------------|
| **security-1** | Security | Auth bypass, injection flaws, cryptographic failures, secrets in code, permission escalation, input validation, insecure dependencies |
| **critical-1** | Critical Bugs | Panics, unwrap on None/Error, index out of bounds, division by zero, data corruption, infinite loops, deadlocks, race conditions |
| **performance-1** | Performance | Memory leaks, unnecessary allocations, slow algorithms (O(n²)+), missing indexes/caches, redundant work, blocking I/O, large clones |
| **reliability-1** | Reliability | Missing error handling, poor error messages, missing retries, no timeouts, unhandled edge cases, incomplete validation, crash-prone code |
| **testing-1** | Testing | Missing tests for critical paths, low coverage on high-risk modules, untested edge cases, flaky test infrastructure, missing test utilities |
| **tech-debt-1** | Technical Debt | Code duplication, dead code, overly complex functions, magic numbers/strings, poor naming, monolithic modules, undocumented public APIs |

**AGENT GUIDELINES:**

1. **security-1**: Focus on vulnerabilities exploitable by attackers. Check: hardcoded secrets (env vars, keys), SQL/NoSQL injection, command injection, path traversal, authentication flaws, authorization bypass, cryptographic misuse, dependency vulnerabilities.

2. **critical-1**: Focus on code that will crash or corrupt data. Check: .unwrap(), .expect(), indexing without bounds, async deadlocks, poison errors, unwatched channels, Rc/Arc cycles.

3. **performance-1**: Focus on resource waste and slow code. Check: unnecessary Vec/HashMap in hot paths, O(n²)+ algorithms, missing .cloned()/.copied(), repeated regex compilation, large trait objects, sync in async contexts.

4. **reliability-1**: Focus on code that works until it doesn't. Check: Result/Option propagation without handling, empty error messages, missing timeouts on network calls, unvalidated inputs, partial failures, resource leaks.

5. **testing-1**: Focus on test coverage gaps. Check: modules handling errors but untested, public APIs without doc tests, edge cases not covered, integration test coverage, fixture quality.

6. **tech-debt-1**: Focus on maintainability barriers. Check: copy-paste code (>3 similar blocks), dead functions/modules, functions >50 lines, variables named "tmp" or "data", magic numbers, undocumented complex logic.

**OUTPUT FORMAT:**

Each agent outputs findings to `docs/outputs/issues/{category}/{filename}.md`:

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
category: security|critical|performance|reliability|testing|tech-debt
files:
- src/file.rs
labels: security|crash|performance|error-handling|testing|debt
---

Additional context if needed...
```

**WORKFLOW:**

1. LAUNCH ALL 6 AGENTS SIMULTANEOUSLY
   - Each agent works independently
   - Each agent outputs to its category folder
   - Wait for ALL agents to complete

2. DEDUPLICATE ACROSS AGENTS
   - Remove duplicates: same title OR same file + similar problem
   - Keep first occurrence, discard duplicates

3. MERGE INTO MAX $ARGUMENTS ISSUES
   If findings > $ARGUMENTS:
   
   **Grouping strategy (in priority order):**
   a. **Same file/module** - group issues in the same code area
   b. **Same category** - security with security, performance with performance
   c. **Same risk level** - group High with High
   d. **Related functionality** - group related features
   
   **Merged issue template:**
   ```
   ---
   title: [P1] Consolidated: {primary_title}
   merged_from: "{count} findings merged"
   problems:
   - {problem 1}
   - {problem 2}
   - {problem N}
   impact: Combined impact on users and system
   reproduction: |
     Finding 1: {reproduction 1}
     Finding 2: {reproduction 2}
   acceptance_criteria:
   - Combined criterion covering all merged issues
   effort: {highest effort among merged}
   risk: {highest risk among merged}
   category: {primary category}
   files:
   - {all unique files}
   labels: {all unique labels from merged findings}
   source_files:
   - docs/outputs/issues/{category}/{original_file1}.md
   - docs/outputs/issues/{category}/{original_file2}.md
   ---
   ```

4. CREATE GITHUB ISSUES
   For each consolidated finding:
   - Title: `[P{1-3}] {title}` (P1=High risk, P2=Medium, P3=Low)
   - Body: Include all merged problems, impacts, reproductions, acceptance criteria
   - Labels: All labels from merged findings
   - Use `gh issue create` for each issue

5. REPORT
   - Issues created: N (max $ARGUMENTS)
   - Findings found: M
   - Findings merged: M - N
   - Repository URL for all issues

**CONSTRAINTS:**
- Maximum GitHub issues: $ARGUMENTS
- If findings <= $ARGUMENTS: create each as separate issue (no merging)
- If findings > $ARGUMENTS: MUST merge into exactly $ARGUMENTS issues
- Never modify problem statements when merging
- Deduplicate across agents first
- One finding = One file = One GitHub issue (or merged equivalent)
- If no files in `docs/outputs/issues/`: report "No issues found" and exit

Begin analysis. Launch all 6 agents simultaneously, deduplicate, merge if needed, create max $ARGUMENTS GitHub issues.
