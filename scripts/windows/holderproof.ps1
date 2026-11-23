param(
    [ValidateSet('setup', 'check', 'test', 'trial', 'start', 'status', 'stop')]
    [string]$Action = 'start',
    [ValidatePattern('^[A-Za-z0-9_.-]+$')]
    [string]$Distribution = 'Ubuntu',
    [ValidateRange(10, 3600)]
    [int]$StartupTimeoutSeconds = 1200,
    [ValidatePattern('^[A-Za-z][A-Za-z0-9_-]{0,15}$')]
    [string]$TesterId = 'E01',
    [ValidateSet('', 'alice', 'bob', 'carol', 'dave')]
    [string]$Holder = '',
    [switch]$Web
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$runtime = Join-Path $repo '.run/windows'
$url = 'http://127.0.0.1:8765'
if (!(Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    throw 'WSL is missing. See docs/guides/WINDOWS_WSL.md.'
}
$wslInput = $repo -replace '\\', '/'
$linuxRepo = & wsl.exe -d $Distribution -- wslpath -a -u $wslInput
if ($LASTEXITCODE -ne 0) { throw "Cannot access WSL distribution $Distribution. See docs/guides/WINDOWS_WSL.md." }
$runner = $linuxRepo.Trim() + '/scripts/windows/runtime.sh'
New-Item -ItemType Directory -Force -Path $runtime | Out-Null

function Get-Snapshot {
    try { Invoke-RestMethod "$url/api/snapshot" -TimeoutSec 3 } catch { $null }
}

if ($Action -in @('setup', 'check', 'test', 'stop')) {
    & wsl.exe -d $Distribution -- bash $runner $Action
    if ($LASTEXITCODE -ne 0) { throw "HolderProof $Action failed. See docs/guides/WINDOWS_WSL.md." }
    exit 0
}

if ($Action -eq 'trial') {
    # 如果之前用启动器开过服务，先自动停掉避免端口冲突
    & wsl.exe -d $Distribution -- bash $runner status | Out-Null
    if ($LASTEXITCODE -eq 0) {
        & wsl.exe -d $Distribution -- bash $runner stop
    }
    $trialArgs = @($TesterId)
    if ($Web) { $trialArgs += '--web' }
    if ($Holder) { $trialArgs += @('--holder', $Holder) }
    # 直接在当前 PowerShell 窗口交互式运行（支持回车确认和输入反馈）
    & wsl.exe -d $Distribution -- bash $runner trial @trialArgs
    if ($LASTEXITCODE -ne 0) { throw "HolderProof trial failed." }
    exit 0
}

& wsl.exe -d $Distribution -- bash $runner status | Out-Null
$owned = ($LASTEXITCODE -eq 0)
$snapshot = Get-Snapshot
if ($owned -and $snapshot) {
    Write-Host "HolderProof is running: $url/"
    [pscustomobject]@{
        Block = $snapshot.certificate.block_number
        Balance = $snapshot.certificate.balance
        CertificateStatus = $snapshot.certificate.status
        Signers = @($snapshot.certificate.signer_ids).Count
        AirdropEligible = $snapshot.gates.airdrop.eligible
        GovernanceEligible = $snapshot.gates.governance.eligible
    } | Format-List
    exit 0
}
if ($Action -eq 'status') {
    if ($owned) { Write-Host 'Runtime is starting or unhealthy; inspect .run/windows/*.log.' }
    else { Write-Host 'This checkout has no running Windows runtime.' }
    exit 1
}
if ($owned) { throw 'Runtime is already starting or unhealthy. Inspect logs or run -Action stop first.' }
if ($snapshot) { throw 'Port 8765 serves another runtime. Stop that runtime before starting this checkout.' }

& wsl.exe -d $Distribution -- bash $runner check
if ($LASTEXITCODE -ne 0) { throw 'Environment check failed. Follow docs/guides/WINDOWS_WSL.md.' }
# Keep one WSL session alive for the devnet, HTTP proxies and Web server.
$process = Start-Process -FilePath wsl.exe -WindowStyle Hidden -PassThru `
    -ArgumentList @('-d', $Distribution, '--', 'bash', "`"$runner`"", 'serve') `
    -RedirectStandardOutput (Join-Path $runtime 'stdout.log') `
    -RedirectStandardError (Join-Path $runtime 'stderr.log')
$stdoutLog = Join-Path $runtime 'stdout.log'
$startedAt = Get-Date
$lastReport = $startedAt
$deadline = $startedAt.AddSeconds($StartupTimeoutSeconds)
Write-Host 'Starting HolderProof in WSL2 (first build may take several minutes)...'
do {
    Start-Sleep -Seconds 1
    $snapshot = Get-Snapshot
    $process.Refresh()
    if ($process.HasExited) { throw 'Runtime exited. Inspect .run/windows/stdout.log and stderr.log.' }
    if ($snapshot) { break }
    $now = Get-Date
    if (($now - $lastReport).TotalSeconds -ge 10) {
        $elapsed = [int]($now - $startedAt).TotalSeconds
        $tail = if (Test-Path $stdoutLog) { (Get-Content $stdoutLog -Tail 1 -ErrorAction SilentlyContinue) } else { '' }
        Write-Host ("  [{0}s] {1}" -f $elapsed, $(if ($tail) { $tail } else { 'working...' }))
        $lastReport = $now
    }
} while ((Get-Date) -lt $deadline)
if (!$snapshot) { throw 'Startup timed out; runtime may still be building. Inspect logs, then use -Action status or stop.' }
Write-Host "HolderProof is ready: $url/"
Write-Host 'Stop with: .\scripts\windows\holderproof.ps1 -Action stop'
