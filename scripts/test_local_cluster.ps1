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

function Invoke-Rpc {
    param(
        [string]$RpcAddress,
        [string]$Method,
        [object[]]$Params
    )

    $body = @{
        jsonrpc = "2.0"
        method = $Method
        params = $Params
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

    return $response.result
}

function Invoke-Balance {
    param(
        [string]$RpcAddress,
        [string]$Address
    )

    return [UInt64](Invoke-Rpc -RpcAddress $RpcAddress -Method "aur_getBalance" -Params @($Address))
}

function Convert-HexToBech32m {
    param([string]$Hex)

    $alphabet = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
    $bytes = @()
    for ($offset = 0; $offset -lt $Hex.Length; $offset += 2) {
        $bytes += [Convert]::ToByte($Hex.Substring($offset, 2), 16)
    }
    $values = New-Object System.Collections.Generic.List[int]
    $accumulator = 0
    $bits = 0
    foreach ($byte in $bytes) {
        $accumulator = ($accumulator -shl 8) -bor [int]$byte
        $bits += 8
        while ($bits -ge 5) {
            $bits -= 5
            [void]$values.Add(($accumulator -shr $bits) -band 31)
        }
    }
    if ($bits -gt 0) {
        [void]$values.Add(($accumulator -shl (5 - $bits)) -band 31)
    }

    $hrp = "aur"
    $hrpValues = New-Object System.Collections.Generic.List[int]
    foreach ($character in $hrp.ToCharArray()) {
        [void]$hrpValues.Add(([int][char]$character) -shr 5)
    }
    [void]$hrpValues.Add(0)
    foreach ($character in $hrp.ToCharArray()) {
        [void]$hrpValues.Add(([int][char]$character) -band 31)
    }

    $polymodValues = @(
        [uint64]0x3b6a57b2,
        [uint64]0x26508e6d,
        [uint64]0x1ea119fa,
        [uint64]0x3d4233dd,
        [uint64]0x2a1462b3
    )
    $polymod = [uint64]1
    foreach ($value in ($hrpValues + $values + @(0, 0, 0, 0, 0, 0))) {
        $top = $polymod -shr 25
        $polymod = (($polymod -band [uint64]0x1ffffff) -shl 5) -bxor [uint64]$value
        for ($index = 0; $index -lt 5; $index++) {
            if (($top -shr $index) -band 1) {
                $polymod = $polymod -bxor $polymodValues[$index]
            }
        }
    }
    $checksumConstant = [uint64]0x2bc830a3
    $checksum = $polymod -bxor $checksumConstant
    $checksumValues = @()
    for ($index = 0; $index -lt 6; $index++) {
        $checksumValues += [int](($checksum -shr (5 * (5 - $index))) -band 31)
    }

    $encoded = ""
    foreach ($value in ($values + $checksumValues)) {
        $encoded += $alphabet[$value]
    }
    return "$hrp`1$encoded"
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
    $buildOutput = & cmd.exe /d /c "cargo build --bin aurion 2>&1"
    $buildExitCode = $LASTEXITCODE
    $buildOutput | ForEach-Object { Write-Host $_.ToString() }
    if ($buildExitCode -ne 0) {
        throw "cargo build failed with exit code $buildExitCode"
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

    $testRecipientHex = "9999999999999999999999999999999999999999999999999999999999999999"
    $recipientAddress = Convert-HexToBech32m -Hex $testRecipientHex
    $transferAmount = [UInt64]1000000000
    $transferFee = [UInt64]10000
    $initialBalance = Invoke-Balance -RpcAddress "127.0.0.1:8548" -Address $recipientAddress
    Write-ClusterLog ("Transfer baseline: recipient {0} balance={1} Quanta." -f $recipientAddress, $initialBalance)
    if ($initialBalance -ne 0) {
        throw "Transfer baseline mismatch: expected recipient balance 0, got $initialBalance."
    }

    $transferStartHeight = [UInt64](($heightEvidence[-1] | Measure-Object -Minimum).Minimum)
    $walletArgs = @(
        "wallet", "send",
        "--rpc", "http://127.0.0.1:8545",
        "--to", $recipientAddress,
        "--amount", $transferAmount,
        "--fee", $transferFee,
        "--yes", "--dev-sender"
    )
    Write-ClusterLog ("Transfer: sending {0} Quanta + {1} Quanta fee from deterministic developer sender." -f $transferAmount, $transferFee)
    $transferStdoutPath = Join-Path $logRoot "wallet_transfer.out.log"
    $transferStderrPath = Join-Path $logRoot "wallet_transfer.err.log"
    $transferProcess = Start-Process `
        -FilePath $binaryPath `
        -ArgumentList $walletArgs `
        -WorkingDirectory $repoRoot `
        -RedirectStandardOutput $transferStdoutPath `
        -RedirectStandardError $transferStderrPath `
        -Wait `
        -PassThru
    $transferExitCode = $transferProcess.ExitCode
    $transferOutput = ""
    if (Test-Path $transferStdoutPath) {
        $transferOutput += Get-Content -LiteralPath $transferStdoutPath -Raw
    }
    if (Test-Path $transferStderrPath) {
        $transferOutput += Get-Content -LiteralPath $transferStderrPath -Raw
    }
    $transferOutput | Write-Host
    if ($transferExitCode -ne 0) {
        throw "wallet send failed with exit code $transferExitCode."
    }
    $txMatch = [regex]::Match($transferOutput, "TxID\s+:\s+0x([0-9a-fA-F]+)")
    if (-not $txMatch.Success) {
        throw "wallet send did not emit a TxID."
    }
    Write-ClusterLog ("Transfer broadcast accepted: TxID=0x{0}." -f $txMatch.Groups[1].Value)

    $confirmationDeadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $confirmedBalance = $initialBalance
    while ((Get-Date) -lt $confirmationDeadline) {
        $confirmedHeights = @()
        foreach ($node in $nodeDefinitions) {
            $confirmedHeights += Invoke-BlockHeight -RpcAddress $node.Rpc
        }
        $confirmedBalance = Invoke-Balance -RpcAddress "127.0.0.1:8548" -Address $recipientAddress
        Write-ClusterLog ("Transfer confirmation: heights={0}/{1}/{2}/{3}, node3 balance={4} Quanta." -f $confirmedHeights[0], $confirmedHeights[1], $confirmedHeights[2], $confirmedHeights[3], $confirmedBalance)
        if (($confirmedHeights | Measure-Object -Minimum).Minimum -ge ($transferStartHeight + 2) -and $confirmedBalance -eq $transferAmount) {
            break
        }
        Start-Sleep -Seconds 1
    }

    if ($confirmedBalance -ne $transferAmount) {
        throw "Transfer confirmation failed: expected node3 balance $transferAmount, got $confirmedBalance."
    }
    Write-ClusterLog ("PASS: Node 3 balance increased exactly to {0} Quanta after two committed blocks." -f $confirmedBalance)

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
