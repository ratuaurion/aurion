[CmdletBinding()]
param(
    [int]$TimeoutSeconds = 60,
    [switch]$KeepData
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$binaryPath = Join-Path $repoRoot "target\debug\aurion.exe"
$clusterRoot = Join-Path $repoRoot ".data"
$logRoot = Join-Path $clusterRoot "cluster_logs"
$processes = @()
$nodeDefinitions = @(
    @{ Index = 0; P2P = "127.0.0.1:7001"; Rpc = "127.0.0.1:8545"; Peer = $null },
    @{ Index = 1; P2P = "127.0.0.1:7002"; Rpc = "127.0.0.1:8546"; Peer = "tcp/127.0.0.1:7001" },
    @{ Index = 2; P2P = "127.0.0.1:7003"; Rpc = "127.0.0.1:8547"; Peer = "tcp/127.0.0.1:7001" },
    @{ Index = 3; P2P = "127.0.0.1:7004"; Rpc = "127.0.0.1:8548"; Peer = "tcp/127.0.0.1:7001" }
)

function Write-ClusterLog {
    param([string]$Message)
    $line = "[{0}] {1}" -f (Get-Date -Format "HH:mm:ss"), $Message
    Write-Host $line
}

function Invoke-BlockHeight {
    param([string]$RpcAddress)

    $body = @{
        jsonrpc = "2.0"
        method = "aur_blockHeight"
        params = @()
        id = 1
    } | ConvertTo-Json -Compress

    $response = Invoke-RestMethod `
        -Uri ("http://{0}/" -f $RpcAddress) `
        -Method Post `
        -Body $body `
        -ContentType "application/json" `
        -TimeoutSec 3

    if ($null -ne $response.error) {
        throw "RPC $RpcAddress returned error: $($response.error.message)"
    }

    return [UInt64]$response.result
}

function Stop-ClusterProcess {
    param([System.Diagnostics.Process]$Process)

    if ($null -eq $Process -or $Process.HasExited) {
        return
    }

    try {
        if ($Process.MainWindowHandle -ne 0) {
            [void]$Process.CloseMainWindow()
            if (-not $Process.WaitForExit(3000)) {
                Stop-Process -Id $Process.Id -Force
            }
        } else {
            Stop-Process -Id $Process.Id -Force
        }
    } catch {
        if (-not $Process.HasExited) {
            Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        }
    }
}

try {
    Set-Location $repoRoot
    Write-ClusterLog "Pre-flight: stopping stale aurion processes."
    Get-Process -Name "aurion" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

    if (Test-Path $clusterRoot) {
        Write-ClusterLog "Pre-flight: removing previous .data directory."
        Remove-Item -LiteralPath $clusterRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $logRoot -Force | Out-Null

    Write-ClusterLog "Building debug binary: cargo build --bin aurion."
    & cargo build --bin aurion
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
    if (-not (Test-Path $binaryPath)) {
        throw "Built binary not found at $binaryPath"
    }

    foreach ($node in $nodeDefinitions) {
        $nodeData = Join-Path $clusterRoot ("cluster_node_{0}" -f $node.Index)
        $stdoutPath = Join-Path $logRoot ("node_{0}.out.log" -f $node.Index)
        $stderrPath = Join-Path $logRoot ("node_{0}.err.log" -f $node.Index)
        $arguments = @(
            "validator", "start", "--dev", "--index", $node.Index,
            "--p2p-bind", $node.P2P,
            "--rpc-bind", $node.Rpc,
            "--data-dir", $nodeData
        )
        if ($null -ne $node.Peer) {
            $arguments += @("--peer", $node.Peer)
        }

        Write-ClusterLog ("Starting node {0}: P2P {1}, RPC {2}, data {3}" -f $node.Index, $node.P2P, $node.Rpc, $nodeData)
        $process = Start-Process `
            -FilePath $binaryPath `
            -ArgumentList $arguments `
            -WorkingDirectory $repoRoot `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath `
            -PassThru
        $processes += $process

        if ($node.Index -eq 0) {
            Start-Sleep -Seconds 2
        }
    }

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $ready = @{}
    while ((Get-Date) -lt $deadline) {
        foreach ($node in $nodeDefinitions) {
            if (-not $ready.ContainsKey($node.Index)) {
                try {
                    $height = Invoke-BlockHeight -RpcAddress $node.Rpc
                    $ready[$node.Index] = $height
                    Write-ClusterLog ("Node {0} RPC ready at height {1}." -f $node.Index, $height)
                } catch {
                    # The listener may still be starting; retry until the deadline.
                }
            }
        }
        if ($ready.Count -eq $nodeDefinitions.Count) {
            break
        }
        Start-Sleep -Milliseconds 500
    }

    if ($ready.Count -ne $nodeDefinitions.Count) {
        throw "RPC readiness timeout: $($ready.Count)/$($nodeDefinitions.Count) nodes responded."
    }

    $heightEvidence = @()
    while ((Get-Date) -lt $deadline) {
        $snapshot = @()
        foreach ($node in $nodeDefinitions) {
            $height = Invoke-BlockHeight -RpcAddress $node.Rpc
            $snapshot += [UInt64]$height
        }
        $heightEvidence += ,@($snapshot)
        Write-ClusterLog ("Heights: node0={0}, node1={1}, node2={2}, node3={3}" -f $snapshot[0], $snapshot[1], $snapshot[2], $snapshot[3])

        if (($snapshot | Measure-Object -Minimum).Minimum -ge 2) {
            Write-ClusterLog "PASS: all four validators reached finalized height >= 2."
            break
        }
        Start-Sleep -Seconds 1
    }

    if ($heightEvidence.Count -eq 0 -or ($heightEvidence[-1] | Measure-Object -Minimum).Minimum -lt 2) {
        throw "Consensus timeout: all validators did not reach height 2 within $TimeoutSeconds seconds."
    }

    Write-ClusterLog "Evidence logs: $logRoot"
    Write-Host "LOCAL_CLUSTER_STATUS=PASS"
} catch {
    Write-Error ("LOCAL_CLUSTER_STATUS=FAIL: {0}" -f $_.Exception.Message)
    if (Test-Path $logRoot) {
        Write-Host "Cluster logs: $logRoot"
    }
    exit 1
} finally {
    Write-ClusterLog "Teardown: stopping cluster processes."
    foreach ($process in $processes) {
        Stop-ClusterProcess -Process $process
    }
    Get-Process -Name "aurion" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

    if (-not $KeepData -and (Test-Path $clusterRoot)) {
        Remove-Item -LiteralPath $clusterRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
