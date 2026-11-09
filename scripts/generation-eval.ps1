#Requires -Version 5.1
<#
.SYNOPSIS
    Cassette-backed DAG generation gate evaluation (P2-25).
.PARAMETER Runs
    Repeat count per chain (default 1).
.PARAMETER OutFile
    Markdown report path (default docs/reviews/generation-eval.md).
#>
[CmdletBinding()]
param(
    [int]$Runs = 1,
    [string]$OutFile = ''
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/lib/aitest-run.ps1"

$repo = Get-RepoRoot
if (-not $OutFile) {
    $OutFile = Join-Path $repo 'docs/reviews/generation-eval.md'
}

$env:GENERATION_EVAL_OUT = $OutFile
$env:GENERATION_EVAL_ITERATIONS = "$Runs"
Invoke-RepoCargo -Args @(
    'test', '-p', 'generator', 'generation_eval_write_report_when_env_set', '--',
    '--exact', '--nocapture'
) -RepoRoot $repo
Write-Host "wrote $OutFile"
