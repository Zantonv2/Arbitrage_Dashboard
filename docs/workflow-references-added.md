# Workflow References Enhancement

## Overview

Enhanced all GitHub workflows to reference project documentation and issue templates, ensuring OpenCode creates properly formatted issues and follows project standards.

## Changes Made

### 1. Code Review Workflow (`code-review.yml`)

**Added References to Issue Templates:**
- `.github/ISSUE_TEMPLATE/bug_report.md` - For bugs, crashes, errors
- `.github/ISSUE_TEMPLATE/security_report.md` - For security vulnerabilities  
- `.github/ISSUE_TEMPLATE/feature_request.md` - For improvements/enhancements

**Enhanced Issue Creation Format:**

Now creates three types of properly formatted issues:

#### Bug/Crash Issues (P1-P2)
```yaml
Title: [P1] Brief description of bug (<70 chars)
Body:
  - Bug Description
  - Steps to Reproduce
  - Expected Behavior
  - Actual Behavior
  - Component (Core Engine/Exchange Connector/Server/Configuration)
  - Relevant Logs
  - Files Affected
  - Additional Context
Labels: bug, auto-generated, code-review
```

#### Security Issues (P1)
```yaml
Title: [P1][Security] Brief description (<70 chars)
Body:
  - Vulnerability Description
  - Impact (data breach, unauthorized access, etc)
  - Affected Components
  - Reproduction Steps
  - Severity (Critical/High/Medium/Low)
  - Files Affected
  - Recommended Fix
Labels: security, auto-generated, code-review
```

#### Performance/Testing/Tech Debt (P2-P3)
```yaml
Title: [P2] Brief description (<70 chars)
Body:
  - Problem
  - Impact
  - Component
  - Files Affected
  - Proposed Solution
  - Additional Context
Labels: performance/testing/debt, auto-generated, code-review
```

### 2. Auto-Fix Workflow (`auto-fix.yml`)

**Added References to Project Documentation:**
- `docs/architecture.md` - System architecture and design principles
- `docs/coding-standards.md` - Coding standards and best practices
- `docs/contributing.md` - Contribution guidelines
- `README.md` - Project overview

**Added Critical Rules Section:**
1. **No Unwrap Rule**: NEVER use `.unwrap()` - always use proper error handling
2. **Financial Safety**: Use `rust_decimal::Decimal` for all financial calculations (NEVER f64)
3. **Error Propagation**: Use `Result<T, ArbitrageError>` for all fallible operations
4. **Testing**: Add/update tests for any code changes
5. **Documentation**: Update docs if changing public APIs

### 3. PR Review Workflow (`pr-review.yml`)

**Added References to Project Documentation:**
- `docs/architecture.md` - System architecture and design principles
- `docs/coding-standards.md` - Coding standards and best practices
- `docs/contributing.md` - Contribution guidelines

**Added Critical Rules to Check:**
1. **No Unwrap Rule**: Flag any `.unwrap()` usage
2. **Financial Safety**: Verify `rust_decimal::Decimal` used for financial calculations
3. **Error Propagation**: Check `Result<T, ArbitrageError>` used for fallible operations
4. **Testing**: Verify tests are included for new functionality
5. **Documentation**: Check if public API changes have documentation

**Applied to Both Jobs:**
- `review-pr` - Scheduled daily PR reviews
- `review-on-pr` - Manual `/oc review` triggered reviews

## Benefits

### 1. Consistent Issue Format
- All auto-generated issues now follow your established templates
- Easier to triage and prioritize issues
- Better structured information for developers

### 2. Context-Aware Fixes
- OpenCode now understands project architecture before making changes
- Follows coding standards automatically
- Respects critical rules (no unwrap, Decimal for finance, etc)

### 3. Better Code Reviews
- Reviews check against documented standards
- Flags violations of critical rules
- Provides consistent feedback based on project guidelines

### 4. Improved Quality
- Issues include all necessary information (reproduction steps, impact, files)
- Fixes follow established patterns and conventions
- Reviews catch common mistakes early

## File Reference Syntax

The workflows use OpenCode's file reference syntax:
```
#[[file:path/to/file.md]]
```

This tells OpenCode to read and understand the referenced file before executing its task.

## Priority Levels

Issues are created with clear priority levels:

- **P1 (Critical)**: 
  - Security vulnerabilities
  - Crashes and panics
  - Data corruption bugs
  - Unwrap usage in production code

- **P2 (High)**:
  - Performance issues
  - Missing tests
  - Error handling gaps
  - Resource leaks

- **P3 (Medium)**:
  - Code quality improvements
  - Refactoring opportunities
  - Tech debt
  - Documentation updates

## Labels Used

Consistent labeling for easy filtering:

- `bug` - Bugs, crashes, errors
- `security` - Security vulnerabilities
- `performance` - Performance issues
- `error-handling` - Missing error handling
- `testing` - Missing tests or test improvements
- `debt` - Tech debt, refactoring
- `auto-generated` - All automated issues
- `code-review` - Issues from code review workflow

## Testing the Changes

### Test Issue Creation:
```bash
# Trigger code review workflow
gh workflow run code-review.yml

# Wait 2-3 minutes, then check issues
gh issue list --label "auto-generated,code-review"
```

### Test Auto-Fix with References:
```bash
# Create a test issue
gh issue create \
  --title "[P2] Test issue for auto-fix" \
  --body "Test if auto-fix reads documentation" \
  --label "bug,auto-generated"

# Trigger auto-fix
gh workflow run auto-fix.yml

# Check if it created a PR
gh pr list --label "auto-generated"
```

### Test PR Review with Standards:
```bash
# Create a test PR with intentional issues
git checkout -b test/standards-check
echo "let x = 1.5; // Using f64 instead of Decimal" >> test.rs
git add . && git commit -m "test: standards check"
git push origin test/standards-check
gh pr create --title "test: standards check" --body "Testing"

# Comment to trigger review
gh pr comment --body "/oc review"

# Check if review flags the f64 usage
```

## Maintenance

### Updating References

If you add new documentation or templates:

1. Update the workflow file's prompt section
2. Add the file reference: `#[[file:path/to/new-doc.md]]`
3. Test the workflow to ensure it reads the new reference

### Updating Issue Formats

If you change issue templates:

1. Update the corresponding section in `code-review.yml`
2. Match the new template structure
3. Test issue creation to verify format

## Next Steps

1. ✅ References added to all workflows
2. ⚠️ Test issue creation with new format
3. ⚠️ Verify auto-fix follows coding standards
4. ⚠️ Check PR reviews flag critical rule violations
5. ⚠️ Monitor issue quality over next week

## Related Documentation

- [GitHub Workflows Fix Guide](./github-workflows-fix-guide.md) - Complete troubleshooting guide
- [Workflow Setup Checklist](../WORKFLOW-SETUP-CHECKLIST.md) - Quick setup steps
- [Architecture](./architecture.md) - System architecture
- [Coding Standards](./coding-standards.md) - Coding standards
- [Contributing](./contributing.md) - Contribution guidelines
