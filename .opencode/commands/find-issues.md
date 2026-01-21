---
description: Analyze codebase, validate findings, and create max $ARGUMENTS GitHub issues
agent: build
subtask: false
---

You are an Engineering Insight Orchestrator. Analyze codebase, validate findings, and create at most $ARGUMENTS GitHub issues.

**FLAGS:**
- **$ARGUMENTS**: Maximum GitHub issues to create (default: 10)
- If agents find fewer issues than $ARGUMENTS, create all found issues
- If agents find more issues than $ARGUMENTS, MERGE similar findings

**AGENTS:**

Launch these 6 specialized agents in parallel:

| Session ID | Focus Area | What to Find |
|------------|------------|--------------|
| **security-1** | Security | Auth bypass, injection, crypto failures, secrets, input validation, insecure deps |
| **critical-1** | Critical Bugs | Panics, unwrap on None/Error, OOB access, division by zero, data corruption, deadlocks |
| **performance-1** | Performance | Memory leaks, unnecessary allocations, slow algos, missing caches, redundant work |
| **reliability-1** | Reliability | Missing error handling, poor errors, missing retries, unhandled edge cases |
| **testing-1** | Testing | Missing tests for critical paths, low coverage modules, untested edge cases |
| **tech-debt-1** | Technical Debt | Code duplication, dead code, complex functions, magic numbers, poor naming |

**AGENT GUIDELINES:**

1. **security-1**: Hardcoded secrets, SQL/NoSQL injection, command injection, path traversal, auth flaws, crypto misuse.
2. **critical-1**: .unwrap(), .expect(), indexing without bounds, async deadlocks, poison errors, unwatched channels.
3. **performance-1**: Unnecessary Vec/HashMap in hot paths, O(n²)+ algos, missing .cloned()/.copied(), repeated regex compilation.
4. **reliability-1**: Result/Option propagation, empty error messages, missing timeouts, unvalidated inputs, resource leaks.
5. **testing-1**: Modules handling errors but untested, public APIs without doc tests, edge cases not covered.
6. **tech-debt-1**: Copy-paste code (>3 similar blocks), dead functions, functions >50 lines, magic numbers, undocumented logic.

**OUTPUT FORMAT:**

Each agent outputs to `docs/outputs/issues/{category}/{filename}.md`:

```
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

**WORKFLOW:**

1. LAUNCH ALL 6 AGENTS SIMULTANEOUSLY
   - Each outputs to its category folder
   - Wait for ALL to complete

2. DEDUPLICATE ACROSS AGENTS
   - Remove: same title OR same file + similar problem
   - Keep first occurrence

3. MERGE INTO MAX $ARGUMENTS ISSUES (if findings > $ARGUMENTS)
   - Group by: same file → same category → same risk → related functionality
   - Apply merged template from below

4. VALIDATOR LOOP (MAX 3 ROUNDS)

   Create validation artifacts directory: `docs/outputs/validation/`

   **ROUND 1: Content Validator** (session: validator-content-1)
   - Review ALL proposed issues (deduplicated + merged)
   - Output to `docs/outputs/validation/round1_content_review.yaml`
   
   **Validator checks:**
   - Problem statement: Clear, specific, actionable?
   - Impact: User-facing and measurable?
   - Reproduction: Steps are reproducible?
   - Acceptance criteria: Testable, complete, covers edge cases?
   - Context: Enough background for a developer to understand?
   - Missing information: What else is needed?

   **Content validator output format:**
   ```yaml
   round: 1
   validator: content
   findings:
     - file: docs/outputs/issues/critical/unwrap_bug.md
       title: "[P1] Remove unwrap in user parsing"
       score: 6/10
       issues:
         - "Problem is vague - which unwrap?"
         - "Missing: which function, which line"
         - "Impact should mention: crashes on malformed input"
         - "AC: Add test for malformed input, verify graceful handling"
       suggestions:
         - "Include stack trace or panic message"
         - "Link to specific line in code"
         - "Add example of malformed input"
       improved_version: |
         title: "[P1] Remove unwrap in User::from_str() at src/user/parser.rs:45"
         problem: "User::from_str() calls .unwrap() on Result from serde, causing panic on malformed input"
         impact: "Users sending malformed input cause panic, resulting in 500 errors and service disruption"
         reproduction: |
           curl -X POST https://api/users -d '{"invalid"}'
           Result: panic at src/user/parser.rs:45
         acceptance_criteria:
           - "Function returns Err(ParseError) instead of panicking"
           - "ParseError includes field name and reason"
           - "Integration test verifies graceful error for malformed input"
         context: |
           This function parses JSON body into User struct. Serde returns Result which is unwrapped.
           The function is called from the user creation endpoint.
         effort: 2
         risk: High
         category: critical
         files:
           - src/user/parser.rs
         labels:
           - bug
           - crash
     - file: ...
       ...
   pass_threshold: 7/10
   ```

   **ROUND 2: Structure Validator** (session: validator-structure-1)
   - Review ALL proposed issues
   - Output to `docs/outputs/validation/round2_structure_review.yaml`
   
   **Validator checks:**
   - Title format: `[P1-P3]` prefix, under 70 chars?
   - Labels: Correct and consistent?
   - Files: Accurate paths, all affected files listed?
   - Risk: Appropriate risk level?
   - Effort: Reasonable effort estimate?
   - YAML: Valid frontmatter?
   - Duplicates: Any duplicate with other findings?

   **Structure validator output format:**
   ```yaml
   round: 2
   validator: structure
   findings:
     - file: docs/outputs/issues/critical/unwrap_bug.md
       title: "[P1] Remove unwrap in user parsing"
       score: 8/10
       issues:
         - "Missing src/user/handler.rs in files (calls the function)"
         - "Label 'security' is incorrect, use 'bug' or 'crash'"
         - "Effort should be 2, not 1"
       suggestions:
         - "Add handler.rs to files list"
         - "Change label to bug, crash"
         - "Update effort to 2"
       improved_version:
         title: "[P1] Remove unwrap in User::from_str() at src/user/parser.rs:45"
         effort: 2
         labels:
           - bug
           - crash
         files:
           - src/user/parser.rs
           - src/user/handler.rs
     - file: ...
       ...
   pass_threshold: 8/10
   ```

   **ROUND 3: Integration Validator** (session: validator-integration-1)
   - Check combined improvements from rounds 1 & 2
   - Output to `docs/outputs/validation/round3_integration_review.yaml`
   - Verify improvements were applied correctly
   - Check for new issues introduced by changes

   **Integration validator output format:**
   ```yaml
   round: 3
   validator: integration
   summary:
     total_findings: 15
     passed: 12
     needs_improvement: 3
     failed: 0
   findings:
     - file: docs/outputs/issues/critical/unwrap_bug.md
       status: passed
       content_score: 8/10
       structure_score: 9/10
       final_suggestions: null
       can_create_issue: true
     - file: docs/outputs/issues/security/secret.md
       status: needs_improvement
       content_score: 5/10
       structure_score: 7/10
       final_suggestions:
         - "Problem statement still vague - specify which env var"
         - "Add reproduction steps with example secret pattern"
       can_create_issue: false
       improvements_needed:
         - "Clarify which environment variable contains the secret"
         - "Add example of secret pattern found"
         - "Provide remediation approach"
     - file: ...
       ...
   ```

   **VALIDATION DECISION:**
   - All findings must pass integration (can_create_issue: true)
   - If any finding fails, improve it and re-run validation (max 3 rounds)
   - After 3 rounds, create issues for passed findings only

5. CREATE GITHUB ISSUES
   For each validated finding:
   - Use improved versions from validation
   - Title: `[P{1-3}] {title}`
   - Body: Include problem, impact, reproduction, AC, context
   - Labels: All validated labels
   - Use `gh issue create`

6. REPORT
   - Issues created: N (max $ARGUMENTS)
   - Findings found: M
   - Findings merged: M - N
   - Validation rounds: K
   - Issues failed validation: F
   - Repository URL

**MERGED ISSUE TEMPLATE:**

```
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

**CONSTRAINTS:**
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
