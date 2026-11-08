#Requires -Version 5.1
<#
.SYNOPSIS
    Single quality gate for the repository. Must pass before every commit.
.PARAMETER Only
    Run a subset of steps: fmt, clippy, test, guards.
#>
[CmdletBinding()]
param(
    [ValidateSet('fmt', 'clippy', 'test', 'guards')]
    [string[]]$Only = @('fmt', 'clippy', 'test', 'guards')
)

$ErrorActionPreference = 'Continue'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot

# Keep in sync with scripts/check.sh and AGENTS.md section 5.
$deterministicCrates = @('common', 'model', 'gate', 'store', 'classifier', 'engine', 'cli')
$forbiddenCrates = @('llm-gateway', 'generator', 'autonomy')
$sleepAllowedFiles = @('crates/common/src/clock.rs')
$sleepAllowedPrefixes = @('crates/mock-sut/')
$sleepPattern = '\b(thread|time)::sleep\b|\btime::\{[^}]*\bsleep\b|\bsleep_until\b'

$failed = New-Object System.Collections.Generic.List[string]

function Invoke-Step([string]$Name, [scriptblock]$Body) {
    Write-Host "==> $Name" -ForegroundColor Cyan
    $ok = & $Body
    if (-not $ok) {
        $failed.Add($Name)
        Write-Host "    FAILED: $Name" -ForegroundColor Red
    }
}

function Invoke-Native([scriptblock]$Command) {
    # Output must not reach the pipeline: it would become part of the return value, and a
    # non-empty array is truthy even when the command failed.
    & $Command | Out-Host
    return ($LASTEXITCODE -eq 0)
}

function Test-DependencyGuard {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $members = @($metadata.packages | ForEach-Object { $_.name })
    $ok = $true
    foreach ($crate in $deterministicCrates) {
        if ($members -notcontains $crate) { continue }
        $tree = cargo tree -p $crate -e normal,build --prefix none --format '{p}'
        if ($LASTEXITCODE -ne 0) { return $false }
        foreach ($forbidden in $forbiddenCrates) {
            $hit = $tree | Select-String -Pattern ('^' + [regex]::Escape($forbidden) + ' v') -Quiet
            if ($hit) {
                Write-Host "    '$crate' depends on forbidden crate '$forbidden'" -ForegroundColor Red
                $ok = $false
            }
        }
    }
    return $ok
}

function Invoke-WebNpm {
    param([string]$Script)
    $web = Join-Path $repoRoot 'web'
    if (-not (Test-Path (Join-Path $web 'package.json'))) {
        return $true
    }
    Push-Location $web
    try {
        npm run $Script --silent | Out-Host
        return ($LASTEXITCODE -eq 0)
    }
    finally {
        Pop-Location
    }
}

function Test-SleepGuard {
    $ok = $true
    $files = Get-ChildItem -Path 'crates' -Recurse -Filter '*.rs' -File -ErrorAction SilentlyContinue
    foreach ($file in $files) {
        $relative = $file.FullName.Substring($repoRoot.Length + 1).Replace('\', '/')
        if ($relative -match '(^|/)target/') { continue }
        if ($sleepAllowedFiles -contains $relative) { continue }
        if ($sleepAllowedPrefixes | Where-Object { $relative.StartsWith($_) }) { continue }
        $hits = Select-String -Path $file.FullName -Pattern $sleepPattern
        foreach ($hit in $hits) {
            Write-Host "    bare sleep at ${relative}:$($hit.LineNumber): $($hit.Line.Trim())" -ForegroundColor Red
            $ok = $false
        }
    }
    return $ok
}

if ($Only -contains 'fmt') {
    Invoke-Step 'cargo fmt' { Invoke-Native { cargo fmt --all -- --check } }
    Invoke-Step 'web: prettier' { Invoke-WebNpm 'format:check' }
}
if ($Only -contains 'clippy') {
    Invoke-Step 'cargo clippy' { Invoke-Native { cargo clippy --workspace --all-targets -- -D warnings } }
    Invoke-Step 'web: eslint' { Invoke-WebNpm 'lint' }
}
if ($Only -contains 'test') {
    Invoke-Step 'cargo test' { Invoke-Native { cargo test --workspace } }
    Invoke-Step 'web: vitest' { Invoke-WebNpm 'test' }
    Invoke-Step 'web: generated types' { Invoke-WebNpm 'check:types' }
    Invoke-Step 'web: build' { Invoke-WebNpm 'build' }
}
if ($Only -contains 'guards') {
    Invoke-Step 'guard: deterministic crates must not depend on LLM crates' { Test-DependencyGuard }
    Invoke-Step 'guard: no bare sleep outside Sleeper' { Test-SleepGuard }
}

if ($failed.Count -gt 0) {
    Write-Host "`ncheck failed: $($failed -join '; ')" -ForegroundColor Red
    exit 1
}
Write-Host "`nall checks passed" -ForegroundColor Green
exit 0
