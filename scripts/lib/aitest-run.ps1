# Shared helpers for spike scripts (mock-sut + mytool run).

function Get-RepoRoot {
    $root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
    if (-not (Test-Path (Join-Path $root 'Cargo.toml'))) {
        throw "repo root not found at $root"
    }
    return $root
}

function Invoke-RepoCargo {
    param(
        [string[]]$Args,
        [string]$RepoRoot
    )
    Push-Location $RepoRoot
    try {
        $cargo = Get-Command cargo -ErrorAction Stop
        & $cargo.Source @Args
        if ($LASTEXITCODE -ne 0) {
            throw "cargo $($Args -join ' ') failed with $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

function Start-MockSut {
    param(
        [string]$RepoRoot,
        [string]$Listen = '127.0.0.1:18888',
        [string[]]$ExtraArgs = @()
    )
    $argList = @(
        'run', '-p', 'mock-sut', '--',
        '--listen', $Listen,
        '--pay-delay-ms', '80'
    ) + $ExtraArgs

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = 'cargo'
    $psi.Arguments = ($argList -join ' ')
    $psi.WorkingDirectory = $RepoRoot
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true

    $proc = [System.Diagnostics.Process]::Start($psi)
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    $baseUrl = $null
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($proc.HasExited) {
            throw "mock-sut exited early: $($proc.StandardError.ReadToEnd())"
        }
        $line = $proc.StandardOutput.ReadLine()
        if ($null -eq $line) {
            Start-Sleep -Milliseconds 200
            continue
        }
        if ($line -match '^https?://') {
            $baseUrl = $line.Trim()
            break
        }
    }
    if (-not $baseUrl) {
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        throw 'timed out waiting for mock-sut base URL'
    }
    return [PSCustomObject]@{
        Process  = $proc
        BaseUrl  = $baseUrl
        Listen   = $Listen
    }
}

function Stop-MockSut {
    param($Handle)
    if ($null -eq $Handle) { return }
    try {
        if (-not $Handle.Process.HasExited) {
            Stop-Process -Id $Handle.Process.Id -Force
        }
    }
    catch {
        # best-effort
    }
}

function New-RunWorkspace {
    param([string]$BaseUrl)
    $dir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "aitest-spike-$([Guid]::NewGuid().ToString('N').Substring(0, 8))")
    $envDir = Join-Path $dir.FullName 'environments'
    New-Item -ItemType Directory -Path $envDir -Force | Out-Null
    Set-Content -Path (Join-Path $envDir 'local.yaml') -Value "base_url: $BaseUrl`n" -NoNewline
    return $dir.FullName
}

function Invoke-MytoolRun {
    param(
        [string]$RepoRoot,
        [string]$Workspace,
        [string]$DagPath,
        [string[]]$ExtraArgs = @()
    )
    $mytool = Join-Path $RepoRoot 'target/debug/mytool.exe'
    if (-not (Test-Path $mytool)) {
        $mytool = Join-Path $RepoRoot 'target/debug/mytool'
    }
    if (-not (Test-Path $mytool)) {
        Invoke-RepoCargo -Args @('build', '-q', '-p', 'cli') -RepoRoot $RepoRoot
    }
    $db = Join-Path $Workspace 'aitest.db'
    $args = @(
        'run', '--dag', $DagPath, '--env', 'local',
        '--env-root', $Workspace, '--no-llm'
    ) + $ExtraArgs
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $mytool
    $psi.Arguments = ($args -join ' ')
    $psi.WorkingDirectory = $Workspace
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.Environment['AITEST_DB'] = $db
    $proc = [System.Diagnostics.Process]::Start($psi)
    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    $proc.WaitForExit()
    return [PSCustomObject]@{
        ExitCode = $proc.ExitCode
        Stdout   = $stdout
        Stderr   = $stderr
    }
}
