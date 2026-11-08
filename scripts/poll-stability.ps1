#Requires -Version 5.1
<#
.SYNOPSIS
    Run examples/async-poll.yaml repeatedly and summarise poll attempt counts (P0-54).
.PARAMETER Runs
    Number of iterations (default 10).
.PARAMETER JitterMs
    mock-sut pay jitter cap (0 = isolate engine-only variance).
.PARAMETER Seed
    mock-sut RNG seed for jitter.
.PARAMETER OutFile
    Optional path to write a markdown table.
#>
[CmdletBinding()]
param(
    [ValidateRange(1, 100)]
    [int]$Runs = 10,
    [ValidateRange(0, 5000)]
    [int]$JitterMs = 40,
    [uint64]$Seed = 42,
    [string]$OutFile = ''
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/lib/aitest-run.ps1"

$repo = Get-RepoRoot
$dag = Join-Path $repo 'examples/async-poll.yaml'
if (-not (Test-Path $dag)) {
    throw "missing $dag"
}

Invoke-RepoCargo -Args @('build', '-q', '-p', 'mock-sut', '-p', 'cli') -RepoRoot $repo

$extra = @('--jitter-ms', "$JitterMs", '--seed', "$Seed")
$sut = Start-MockSut -RepoRoot $repo -ExtraArgs $extra
$workspace = New-RunWorkspace -BaseUrl $sut.BaseUrl

$rows = New-Object System.Collections.Generic.List[object]
try {
    for ($i = 1; $i -le $Runs; $i++) {
        $reportPath = Join-Path $workspace "run-$i.json"
        $result = Invoke-MytoolRun -RepoRoot $repo -Workspace $workspace -DagPath $dag -ExtraArgs @(
            '--report-json', $reportPath
        )
        $pollAttempts = $null
        $durationMs = $null
        if (Test-Path $reportPath) {
            $report = Get-Content $reportPath -Raw | ConvertFrom-Json
            $pollNode = $report.deterministic.nodes.wait_paid
            if ($pollNode.poll) {
                $pollAttempts = @($pollNode.poll).Count
            }
            $vol = $report.volatile.nodes.wait_paid
            if ($vol.timing.duration_ms) {
                $durationMs = [int]$vol.timing.duration_ms
            }
        }
        $rows.Add([PSCustomObject]@{
                Run            = $i
                ExitCode       = $result.ExitCode
                PollAttempts   = $pollAttempts
                WaitDurationMs = $durationMs
            })
    }
}
finally {
    Stop-MockSut $sut
}

$passed = @($rows | Where-Object { $_.ExitCode -eq 0 }).Count
$attempts = @($rows | Where-Object { $null -ne $_.PollAttempts } | ForEach-Object { $_.PollAttempts })
$attemptMin = if ($attempts.Count) { ($attempts | Measure-Object -Minimum).Minimum } else { $null }
$attemptMax = if ($attempts.Count) { ($attempts | Measure-Object -Maximum).Maximum } else { $null }
$uniqueAttempts = if ($attempts.Count) { ($attempts | Select-Object -Unique).Count } else { 0 }

$jitterSource = if ($JitterMs -eq 0) {
    if ($uniqueAttempts -le 1) { 'engine stable (jitter disabled, constant poll depth)' }
    else { 'engine scheduling variance (jitter disabled but attempt count varies)' }
}
else {
    if ($uniqueAttempts -le 1) { 'mock-sut delay within single backoff window; engine poll cadence stable' }
    else { 'likely mock-sut payment delay jitter (seeded); review volatile poll ts spread' }
}

$summary = @(
    "Poll stability ($Runs runs, jitter_ms=$JitterMs, seed=$Seed)"
    "pass_rate: $passed/$Runs"
    "poll_attempts: min=$attemptMin max=$attemptMax unique=$uniqueAttempts"
    "jitter_source_hypothesis: $jitterSource"
    ''
    '| Run | Exit | PollAttempts | WaitDurationMs |'
    '|---:|---:|---:|---:|'
)

foreach ($r in $rows) {
    $pa = if ($null -eq $r.PollAttempts) { '' } else { $r.PollAttempts }
    $dm = if ($null -eq $r.WaitDurationMs) { '' } else { $r.WaitDurationMs }
    $summary += "| $($r.Run) | $($r.ExitCode) | $pa | $dm |"
}

$text = $summary -join "`n"
Write-Host $text

if ($OutFile) {
    $dir = Split-Path $OutFile -Parent
    if ($dir -and -not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    Set-Content -Path $OutFile -Value $text -Encoding utf8
}

if ($passed -ne $Runs) {
    exit 1
}
exit 0
