# GitHub Workflows Fix Guide

## Problems Identified and Solutions

### Problem 1: Scheduled Workflows Not Running Daily ⏰

#### Root Causes:
1. **GitHub disables scheduled workflows after 60 days of repository inactivity**
2. **Workflows must exist on the default branch** (usually `main`) to run on schedule
3. **High load times** - Workflows scheduled at the start of every hour may be delayed or dropped
4. **Repository activity required** - GitHub automatically disables scheduled workflows in inactive repos

#### Solutions Applied:

✅ **Added `GH_TOKEN` environment variables** to all `gh` CLI commands (they were missing authentication)

✅ **Improved cron timing** - Your current schedules are good (8 AM, 11 AM, 1 PM, 5 PM UTC avoid peak hours)

#### Additional Actions Required:

1. **Verify workflows are enabled**:
   - Go to: `https://github.com/YOUR_USERNAME/YOUR_REPO/actions`
   - Check the left sidebar under "All Workflows"
   - If any workflow shows "disabled", click it and enable it

2. **Ensure workflows are on the default branch**:
   ```bash
   git checkout main
   git pull origin main
   # Verify .github/workflows/*.yml files exist
   ```

3. **Keep repository active**:
   - Make at least one commit every 60 days
   - Or manually trigger workflows using `workflow_dispatch`

4. **Monitor workflow runs**:
   - Check Actions tab daily for the first week
   - Look for any error messages or skipped runs

---

### Problem 2: Bot Comments Don't Trigger Workflows 🤖

#### Root Cause:
**This is a GitHub security feature by design**. When a workflow uses `GITHUB_TOKEN` to create comments, those comments **will not trigger other workflows**. This prevents:
- Infinite workflow loops
- Security vulnerabilities
- Unauthorized workflow executions

From [GitHub Docs](https://docs.github.com/en/actions/security-guides/automatic-token-authentication#using-the-github_token-in-a-workflow):
> "When you use the repository's GITHUB_TOKEN to perform tasks, events triggered by the GITHUB_TOKEN will not create a new workflow run."

#### Solution Applied:

✅ **Updated `auto-trigger-review.yml`** to use a Personal Access Token (PAT) instead of `GITHUB_TOKEN`

#### Required Setup Steps:

1. **Create a Personal Access Token (PAT)**:
   - Go to: https://github.com/settings/tokens
   - Click "Generate new token" → "Generate new token (classic)"
   - Name it: `Workflow Trigger Token`
   - Select scopes:
     - ✅ `repo` (Full control of private repositories)
     - ✅ `workflow` (Update GitHub Action workflows)
   - Click "Generate token"
   - **COPY THE TOKEN** (you won't see it again!)

2. **Add PAT as a repository secret**:
   - Go to: `https://github.com/YOUR_USERNAME/YOUR_REPO/settings/secrets/actions`
   - Click "New repository secret"
   - Name: `PAT_TOKEN`
   - Value: Paste your PAT
   - Click "Add secret"

3. **Test the workflow**:
   - Create a test PR with Rust code changes
   - The `auto-trigger-review.yml` workflow should run
   - It will post `/oc review` comment using the PAT
   - The `pr-review.yml` workflow should then trigger

#### Alternative Solution (If PAT doesn't work):

Instead of trying to trigger workflows via comments, **directly trigger the review workflow** when a PR is opened:

```yaml
# In auto-trigger-review.yml, replace the comment step with:
- name: Trigger PR review workflow directly
  uses: actions/github-script@v7
  with:
    github-token: ${{ secrets.PAT_TOKEN }}
    script: |
      await github.rest.actions.createWorkflowDispatch({
        owner: context.repo.owner,
        repo: context.repo.repo,
        workflow_id: 'pr-review.yml',
        ref: 'main',
        inputs: {
          pr_number: context.payload.pull_request.number.toString()
        }
      });
```

---

### Problem 3: Code Review Workflow Can't Create Issues 📝

#### Root Causes:
1. **Missing `GH_TOKEN` environment variable** for `gh` CLI commands
2. **Flawed logic** - Trying to find files that OpenCode may not create
3. **Wrong approach** - Parsing files instead of having OpenCode create issues directly

#### Solutions Applied:

✅ **Added `GH_TOKEN` to all `gh` CLI commands**

✅ **Updated OpenCode prompt** to create GitHub issues directly using `gh issue create` commands

✅ **Removed complex file-parsing logic** that was trying to find and parse issue files

#### How It Works Now:

The updated `code-review.yml` workflow now:
1. Runs OpenCode with specialized agents
2. **OpenCode creates issues directly** using `gh issue create` commands
3. No intermediate files or parsing needed
4. Issues are created with proper labels and formatting

#### Verify It Works:

1. **Manual test**:
   ```bash
   # Trigger the workflow manually
   gh workflow run code-review.yml
   ```

2. **Check the workflow logs**:
   - Go to Actions tab
   - Click on the "Code Review" workflow run
   - Look for "Created issue #XXX" messages

3. **Check created issues**:
   ```bash
   gh issue list --label "auto-generated,code-review"
   ```

---

## Testing Your Fixes

### Test Schedule Workflows:

```bash
# Manually trigger each scheduled workflow
gh workflow run code-review.yml
gh workflow run auto-fix.yml
gh workflow run pr-review.yml
gh workflow run auto-merge.yml

# Check if they ran successfully
gh run list --limit 5
```

### Test PR Review Trigger:

1. Create a test branch:
   ```bash
   git checkout -b test/workflow-trigger
   echo "// test comment" >> crates/arbitrage-core/src/lib.rs
   git add .
   git commit -m "test: trigger workflow"
   git push origin test/workflow-trigger
   ```

2. Create a PR:
   ```bash
   gh pr create --title "test: workflow trigger" --body "Testing workflow triggers"
   ```

3. Check if workflows run:
   - `auto-trigger-review.yml` should run immediately
   - It should post `/oc review` comment
   - `pr-review.yml` should trigger from the comment

### Test Issue Creation:

1. Run code review manually:
   ```bash
   gh workflow run code-review.yml
   ```

2. Wait 2-3 minutes, then check for issues:
   ```bash
   gh issue list --label "auto-generated"
   ```

---

## Monitoring and Maintenance

### Weekly Checks:

1. **Verify scheduled workflows ran**:
   ```bash
   gh run list --workflow=code-review.yml --limit 7
   gh run list --workflow=auto-fix.yml --limit 7
   gh run list --workflow=pr-review.yml --limit 7
   gh run list --workflow=auto-merge.yml --limit 7
   ```

2. **Check for disabled workflows**:
   - Visit: `https://github.com/YOUR_USERNAME/YOUR_REPO/actions`
   - Look for any "disabled" badges

3. **Review created issues**:
   ```bash
   gh issue list --label "auto-generated" --state open
   ```

### Troubleshooting:

#### If scheduled workflows still don't run:

1. Check if workflows are enabled in the Actions tab
2. Verify workflows exist on the `main` branch
3. Make a commit to keep the repository active
4. Check GitHub Status: https://www.githubstatus.com/

#### If bot comments still don't trigger workflows:

1. Verify `PAT_TOKEN` secret exists and is valid
2. Check token has `repo` and `workflow` scopes
3. Try regenerating the PAT
4. Consider using `workflow_dispatch` trigger instead

#### If issues aren't being created:

1. Check workflow logs for errors
2. Verify `OPENCODE_API_KEY` secret is set
3. Check if OpenCode has permissions to run `gh` commands
4. Look for rate limiting errors (GitHub API has limits)

---

## Summary of Changes Made

### Files Modified:

1. ✅ `.github/workflows/auto-trigger-review.yml` - Now uses PAT for comments
2. ✅ `.github/workflows/code-review.yml` - OpenCode creates issues directly + references issue templates
3. ✅ `.github/workflows/pr-review.yml` - Added `GH_TOKEN` + references to coding standards
4. ✅ `.github/workflows/auto-fix.yml` - Added `GH_TOKEN` + references to architecture/coding docs

### New Features Added:

1. **Issue Template References** - Code review workflow now references:
   - `.github/ISSUE_TEMPLATE/bug_report.md` - For bugs, crashes, errors
   - `.github/ISSUE_TEMPLATE/security_report.md` - For security vulnerabilities
   - `.github/ISSUE_TEMPLATE/feature_request.md` - For improvements/enhancements

2. **Documentation References** - All workflows now reference:
   - `docs/architecture.md` - System architecture and design principles
   - `docs/coding-standards.md` - Coding standards and best practices
   - `docs/contributing.md` - Contribution guidelines
   - `README.md` - Project overview

3. **Enhanced Issue Creation** - Code review workflow now creates properly formatted issues matching your templates with:
   - Structured bug reports with reproduction steps
   - Security vulnerability reports with impact assessment
   - Performance/testing/tech debt issues with clear descriptions

### Required Actions:

1. ⚠️ **Create and add `PAT_TOKEN` secret** (see Problem 2 above)
2. ⚠️ **Enable workflows** if they're disabled in Actions tab
3. ⚠️ **Test workflows** using manual triggers
4. ⚠️ **Monitor for one week** to ensure scheduled runs work

---

## Additional Resources

- [GitHub Actions Scheduled Workflows](https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#schedule)
- [GitHub Token Authentication](https://docs.github.com/en/actions/security-guides/automatic-token-authentication)
- [Creating Personal Access Tokens](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token)
- [GitHub Actions Permissions](https://docs.github.com/en/actions/security-guides/automatic-token-authentication#permissions-for-the-github_token)
