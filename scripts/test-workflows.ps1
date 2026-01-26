# GitHub Workflows Testing Script
# Run this script to test all your workflow fixes

Write-Host "🚀 Testing GitHub Workflows" -ForegroundColor Cyan
Write-Host ""

# Check if gh CLI is installed
Write-Host "Checking GitHub CLI..." -ForegroundColor Yellow
if (!(Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Host "❌ GitHub CLI (gh) is not installed!" -ForegroundColor Red
    Write-Host "Install it from: https://cli.github.com/" -ForegroundColor Yellow
    exit 1
}
Write-Host "✅ GitHub CLI found" -ForegroundColor Green
Write-Host ""

# Check authentication
Write-Host "Checking GitHub authentication..." -ForegroundColor Yellow
$authStatus = gh auth status 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Not authenticated with GitHub!" -ForegroundColor Red
    Write-Host "Run: gh auth login" -ForegroundColor Yellow
    exit 1
}
Write-Host "✅ Authenticated" -ForegroundColor Green
Write-Host ""

# Test 1: Check if workflows are enabled
Write-Host "📋 Test 1: Checking workflow status..." -ForegroundColor Cyan
$workflows = @("code-review.yml", "auto-fix.yml", "pr-review.yml", "auto-merge.yml", "auto-trigger-review.yml")
foreach ($workflow in $workflows) {
    Write-Host "  Checking $workflow..." -ForegroundColor Gray
    $status = gh workflow view $workflow 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✅ $workflow is enabled" -ForegroundColor Green
    } else {
        Write-Host "  ⚠️  $workflow may be disabled or not found" -ForegroundColor Yellow
    }
}
Write-Host ""

# Test 2: Check required secrets
Write-Host "🔐 Test 2: Checking required secrets..." -ForegroundColor Cyan
Write-Host "  Note: Cannot read secret values, only verify they exist" -ForegroundColor Gray
$secrets = gh secret list 2>&1
if ($secrets -match "OPENCODE_API_KEY") {
    Write-Host "  ✅ OPENCODE_API_KEY exists" -ForegroundColor Green
} else {
    Write-Host "  ❌ OPENCODE_API_KEY not found!" -ForegroundColor Red
}
if ($secrets -match "WORKFLOW_TRIGGER_TOKEN") {
    Write-Host "  ✅ WORKFLOW_TRIGGER_TOKEN exists" -ForegroundColor Green
} else {
    Write-Host "  ⚠️  WORKFLOW_TRIGGER_TOKEN not found (required for bot comment triggers)" -ForegroundColor Yellow
    Write-Host "     Create one at: https://github.com/settings/tokens" -ForegroundColor Yellow
}
Write-Host ""

# Test 3: Manually trigger workflows
Write-Host "🎯 Test 3: Manually triggering workflows..." -ForegroundColor Cyan
$response = Read-Host "Do you want to manually trigger the code-review workflow? (y/n)"
if ($response -eq "y") {
    Write-Host "  Triggering code-review.yml..." -ForegroundColor Gray
    gh workflow run code-review.yml
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✅ Workflow triggered successfully" -ForegroundColor Green
        Write-Host "  Check status: gh run list --workflow=code-review.yml --limit 1" -ForegroundColor Gray
    } else {
        Write-Host "  ❌ Failed to trigger workflow" -ForegroundColor Red
    }
}
Write-Host ""

# Test 4: Check recent workflow runs
Write-Host "📊 Test 4: Checking recent workflow runs..." -ForegroundColor Cyan
Write-Host "  Last 5 workflow runs:" -ForegroundColor Gray
gh run list --limit 5
Write-Host ""

# Test 5: Check for auto-generated issues
Write-Host "🐛 Test 5: Checking auto-generated issues..." -ForegroundColor Cyan
$issues = gh issue list --label "auto-generated" --limit 5 2>&1
if ($LASTEXITCODE -eq 0 -and $issues) {
    Write-Host "  ✅ Found auto-generated issues:" -ForegroundColor Green
    gh issue list --label "auto-generated" --limit 5
} else {
    Write-Host "  ℹ️  No auto-generated issues found yet" -ForegroundColor Yellow
    Write-Host "     This is normal if workflows haven't run yet" -ForegroundColor Gray
}
Write-Host ""

# Test 6: Check open PRs
Write-Host "🔀 Test 6: Checking open pull requests..." -ForegroundColor Cyan
$prs = gh pr list --state open --limit 5 2>&1
if ($LASTEXITCODE -eq 0 -and $prs) {
    Write-Host "  ✅ Found open PRs:" -ForegroundColor Green
    gh pr list --state open --limit 5
} else {
    Write-Host "  ℹ️  No open PRs found" -ForegroundColor Yellow
}
Write-Host ""

# Summary
Write-Host "=" * 60 -ForegroundColor Cyan
Write-Host "📝 SUMMARY & NEXT STEPS" -ForegroundColor Cyan
Write-Host "=" * 60 -ForegroundColor Cyan
Write-Host ""
Write-Host "✅ Completed Tests" -ForegroundColor Green
Write-Host ""
Write-Host "⚠️  Required Actions:" -ForegroundColor Yellow
Write-Host "  1. If WORKFLOW_TRIGGER_TOKEN is missing:" -ForegroundColor White
Write-Host "     - Create PAT at: https://github.com/settings/tokens" -ForegroundColor Gray
Write-Host "     - Add as secret: Settings → Secrets → Actions" -ForegroundColor Gray
Write-Host ""
Write-Host "  2. Monitor workflows for the next week:" -ForegroundColor White
Write-Host "     - Check Actions tab daily" -ForegroundColor Gray
Write-Host "     - Verify scheduled runs occur" -ForegroundColor Gray
Write-Host "     - Look for any error messages" -ForegroundColor Gray
Write-Host ""
Write-Host "  3. Test PR trigger by creating a test PR:" -ForegroundColor White
Write-Host "     git checkout -b test/workflow" -ForegroundColor Gray
Write-Host "     echo '// test' >> README.md" -ForegroundColor Gray
Write-Host "     git add . && git commit -m 'test: workflow'" -ForegroundColor Gray
Write-Host "     git push origin test/workflow" -ForegroundColor Gray
Write-Host "     gh pr create --title 'test: workflow' --body 'Testing'" -ForegroundColor Gray
Write-Host ""
Write-Host "📚 Full documentation: docs/github-workflows-fix-guide.md" -ForegroundColor Cyan
Write-Host ""
