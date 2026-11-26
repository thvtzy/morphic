param(
    [ValidateSet("all", "default", "bootstrap", "no-z3")]
    [string]$Config = "all",
    [string]$LogDir = (Join-Path $env:TEMP "morphic-suite-logs")
)

$ErrorActionPreference = "Continue"

$repoRoot = (git rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    Write-Error "Must be run inside the Morphic git repository."
    exit 1
}

Push-Location $repoRoot
try {
    New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

    # Guard against accidental CRLF drift on Windows
    $crlfCount = (git ls-files --eol | Select-String "w/crlf").Count
    if ($crlfCount -gt 0) {
        Write-Warning "Detected $crlfCount tracked file(s) with CRLF line endings! Re-checkout LF before trusting byte-mass or SHA-256 tests."
    } else {
        Write-Host "EOL check: OK (0 tracked files with w/crlf)"
    }

    $configs = switch ($Config) {
        "default"   { @(@{ Name = "default";   Args = @("test", "--no-fail-fast") }) }
        "bootstrap" { @(@{ Name = "bootstrap"; Args = @("test", "--features", "bootstrap-codegen", "--no-fail-fast") }) }
        "no-z3"     { @(@{ Name = "no-z3";     Args = @("test", "--no-default-features", "--no-fail-fast") }) }
        "all"       {
            @(
                @{ Name = "default";   Args = @("test", "--no-fail-fast") },
                @{ Name = "bootstrap"; Args = @("test", "--features", "bootstrap-codegen", "--no-fail-fast") },
                @{ Name = "no-z3";     Args = @("test", "--no-default-features", "--no-fail-fast") }
            )
        }
    }

    $overallExit = 0
    $summaryRows = @()

    foreach ($cfg in $configs) {
        $name = $cfg.Name
        $logFile = Join-Path $LogDir ("suite_" + $name + ".log")
        Write-Host "`n=== Running SOLO: cargo $($cfg.Args -join ' ') ==="
        Write-Host "Log file: $logFile"

        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        & cargo @($cfg.Args) 2>&1 | Tee-Object -FilePath $logFile | Out-Null
        $ec = $LASTEXITCODE
        $sw.Stop()

        $lines = Select-String -Path $logFile -Pattern "^test result:"
        $passed = 0; $failed = 0; $ignored = 0
        foreach ($m in $lines) {
            if ($m.Line -match "(\d+) passed; (\d+) failed; (\d+) ignored") {
                $passed  += [int]$matches[1]
                $failed  += [int]$matches[2]
                $ignored += [int]$matches[3]
            }
        }

        $failedTests = @(Select-String -Path $logFile -Pattern "^test .+ \.\.\. FAILED$" | ForEach-Object { $_.Line.Trim() })
        $elapsed = [math]::Round($sw.Elapsed.TotalSeconds, 1)
        $status = if ($ec -eq 0 -and $failed -eq 0) { "PASS" } else { "FAIL" }
        if ($status -ne "PASS") { $overallExit = 1 }

        $summaryRows += [PSCustomObject]@{
            Config   = $name
            Status   = $status
            Passed   = $passed
            Failed   = $failed
            Ignored  = $ignored
            Binaries = $lines.Count
            Seconds  = $elapsed
            Log      = $logFile
        }

        Write-Host ("[{0}] {1}: {2} passed / {3} failed / {4} ignored ({5} binaries, {6}s)" -f $status, $name, $passed, $failed, $ignored, $lines.Count, $elapsed)
        if ($failedTests.Count -gt 0) {
            Write-Host "Failed tests in ${name}:"
            foreach ($ft in $failedTests) { Write-Host "  $ft" }
            if ($failedTests -match "phase84_symbolic_state_encodability") {
                Write-Warning "Known load flake phase84_symbolic_state_encodability failed. Re-run solo before treating as regression:"
                Write-Host "  cargo test --lib phase84_symbolic_state_encodability -- --exact"
                Write-Host "  cargo test --lib"
            }
        }
    }

    Write-Host "`n=== FINAL SOLO SUITE SUMMARY ==="
    $summaryRows | Format-Table -AutoSize
    exit $overallExit
} finally {
    Pop-Location
}
