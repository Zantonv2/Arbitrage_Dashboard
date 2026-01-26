# GitHub Workflows Setup Checklist

## 🚀 Quick Setup (5 minutes)

### Step 1: Create Personal Access Token (PAT)
- [ ] Go to https://github.com/settings/tokens
- [ ] Click "Generate new token (classic)"
- [ ] Name: `Workflow Trigger Token`
- [ ] Select scopes: `repo` + `workflow`
- [ ] Copy the token (save it somewhere safe!)

### Step 2: Add Token as Repository Secret
- [ ] Go to your repo → Settings → Secrets and variables → Actions
- [ ] Click "New repository secret"
- [ ] Name: `PAT_TOKEN`
- [ ] Paste your PAT token
- [ ] Click "Add secret"

### Step 3: Enable Workflows
- [ ] Go to your repo → Actions tab
- [ ] Check left sidebar for any "disabled" workflows
- [ ] Click each disabled workflow and enable it

### Step 4: Test Workflows
```bash
# Test code review workflow
gh workflow run code-review.yml

# Test auto-fix workflow  
gh workflow run auto-fix.yml

# Check if they ran
gh run list --limit 5
```

### Step 5: Test PR Trigger
```bash
# Create test PR
git checkout -b test/workflow
echo "// test" >> README.md
git add . && git commit -m "test: workflow"
git push origin test/workflow
gh pr create --title "test: workflow" --body "Testing"

# Check if auto-trigger-review.yml runs
# Check if it posts "/oc review" comment
# Check if pr-review.yml triggers
```

## ✅ Verification

### After Setup
- [ ] Test code review creates properly formatted issues
- [ ] Verify issues match your issue templates
- [ ] Check auto-fix references project documentation
- [ ] Confirm PR reviews check against coding standards

### Daily (First Week)
- [ ] Check Actions tab for scheduled workflow runs
- [ ] Verify issues are being created with `auto-generated` label
- [ ] Check PR comments trigger reviews

### Weekly (Ongoing)
- [ ] Verify all 4 scheduled workflows ran this week
- [ ] Review auto-generated issues
- [ ] Check for any failed workflow runs

## 🔧 Troubleshooting

### Scheduled workflows not running?
1. Check if workflows are enabled (Actions tab)
2. Verify workflows exist on `main` branch
3. Make a commit (keeps repo active)
4. Check https://www.githubstatus.com/

### Bot comments not triggering workflows?
1. Verify `PAT_TOKEN` secret exists
2. Check token hasn't expired
3. Regenerate PAT if needed
4. Verify token has `repo` + `workflow` scopes

### Issues not being created?
1. Check workflow logs for errors
2. Verify `OPENCODE_API_KEY` secret is set
3. Check for GitHub API rate limits
4. Look for permission errors in logs

## 📋 Workflow Schedule

| Workflow | Time (UTC) | Frequency | Purpose |
|----------|-----------|-----------|---------|
| code-review.yml | 8:00 AM | Daily | Scan codebase for issues |
| auto-fix.yml | 11:00 AM | Daily | Fix reported issues |
| pr-review.yml | 1:00 PM | Daily | Review open PRs |
| auto-merge.yml | 5:00 PM | Daily | Merge safe PRs |

## 🎯 Success Criteria

- ✅ All 4 workflows run daily without errors
- ✅ Bot comments trigger PR reviews
- ✅ Issues are created automatically from code reviews
- ✅ No "disabled workflow" warnings in Actions tab

## 📚 Full Documentation

- `docs/github-workflows-fix-guide.md` - Detailed troubleshooting and explanations
- `docs/workflow-references-added.md` - Documentation on issue templates and references added
