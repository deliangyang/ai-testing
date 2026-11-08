#Requires -Version 5.1
<#
.SYNOPSIS
    Optional P1-30 spike: run NSH headless test when the CLI is installed.
#>
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot

$nsh = Join-Path $env:LOCALAPPDATA 'nsh\bin\nsh.exe'
if (-not (Test-Path $nsh)) {
    Write-Host 'SKIP: nsh CLI not found (install: https://nsh.dev/docs/installation.md)'
    exit 0
}

Write-Host "==> nsh-bridge unit tests"
cargo test -p nsh-bridge --quiet
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$aitest = Join-Path $repoRoot 'app\aitest'
if (-not (Test-Path (Join-Path $aitest 'nsh.json'))) {
    Write-Host 'SKIP: app/aitest not initialized (see app/README.md)'
    exit 0
}

Write-Host "==> nsh test $aitest"
& $nsh test $aitest
exit $LASTEXITCODE
