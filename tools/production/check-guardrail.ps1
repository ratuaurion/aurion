# Aurion Native PowerShell Guardrail Runner
$ErrorActionPreference = "Stop"
Write-Host "Running Aurion Invariant Guardrail Auditor..." -ForegroundColor Cyan

python tools/guardrail.py
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nFATAL: Guardrail detected invariant violations! Halting execution.`n" -ForegroundColor Red
    exit 1
}

Write-Host "`nRunning Cargo Workspace Unit & Conformance Test Suite..." -ForegroundColor Cyan
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nFATAL: Test suite failed! Halting execution.`n" -ForegroundColor Red
    exit 1
}

Write-Host "`nRunning Cargo Clippy Linter (-D warnings)..." -ForegroundColor Cyan
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nFATAL: Clippy linter warnings/errors detected! Halting execution.`n" -ForegroundColor Red
    exit 1
}

Write-Host "`n=========================================================" -ForegroundColor Green
Write-Host "   AURION SYSTEM HEALTH: 100% CANONICAL & VERIFIED" -ForegroundColor Green
Write-Host "=========================================================`n" -ForegroundColor Green
