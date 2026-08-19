[CmdletBinding()]
param(
    [string]$Model = $(if ($env:DEEPSEEK_MODEL) { $env:DEEPSEEK_MODEL } else { "deepseek-v4-flash" }),
    [string]$BaseUrl = $(if ($env:DEEPSEEK_BASE_URL) { $env:DEEPSEEK_BASE_URL } else { "https://api.deepseek.com" }),
    [string]$InstallDir = $(if ($env:DSH_DESKTOP_INSTALL_DIR) { $env:DSH_DESKTOP_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "DSH Desktop" })
)

$ErrorActionPreference = "Stop"
$apiKey = $env:DEEPSEEK_API_KEY
if ([string]::IsNullOrWhiteSpace($apiKey)) {
    throw "请先在当前 PowerShell 会话设置 DEEPSEEK_API_KEY；不要把 Token 写入脚本或命令行参数。"
}

function Invoke-DeepSeekSmokeTest {
    Write-Host "DeepSeek API smoke test (response is not printed)…"
    $body = @{
        model = $Model
        messages = @(@{ role = "user"; content = "Reply with OK only." })
        max_tokens = 8
        stream = $false
    } | ConvertTo-Json -Depth 5 -Compress
    $headers = @{ Authorization = "Bearer $apiKey" }
    try {
        $reply = Invoke-RestMethod -Method Post -Uri ($BaseUrl.TrimEnd("/") + "/chat/completions") -Headers $headers -Body $body -ContentType "application/json" -TimeoutSec 30
    } catch {
        throw "DeepSeek API 验证失败（模型：$Model）：$($_.Exception.Message)"
    }
    if (-not $reply.choices -or $reply.choices.Count -lt 1) {
        throw "DeepSeek API 未返回有效 choices（模型：$Model）"
    }
    Write-Host "DeepSeek API smoke test: OK ($Model)" -ForegroundColor Green
}

function Find-LocalService {
    foreach ($port in 3080..3180) {
        $client = New-Object System.Net.Sockets.TcpClient
        try {
            $connection = $client.BeginConnect("127.0.0.1", $port, $null, $null)
            if ($connection.AsyncWaitHandle.WaitOne(250) -and $client.Connected) {
                $client.EndConnect($connection)
                return $port
            }
        } catch {
            continue
        } finally {
            $client.Dispose()
        }
    }
    return $null
}

$root = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $root "target\release\dsh-desktop.exe"
$installedBinary = Join-Path $InstallDir "bin\dsh-desktop.exe"
$frontend = Join-Path $root "frontend\dsh-web-ui"
$installedFrontend = Join-Path $InstallDir "frontend\dsh-web-ui"
$process = $null
$previousModel = $env:DEEPSEEK_MODEL
$previousBaseUrl = $env:DEEPSEEK_BASE_URL
$logPath = Join-Path $env:APPDATA "neko233\DSH Desktop\data\dsh.log"
$logLineCount = if (Test-Path -LiteralPath $logPath) { @(Get-Content -LiteralPath $logPath -ErrorAction SilentlyContinue).Count } else { 0 }

try {
    Write-Host "Building release binary…"
    cargo build --release --locked --manifest-path (Join-Path $root "Cargo.toml")
    if (-not (Test-Path -LiteralPath $binary)) { throw "未找到 release binary：$binary" }
    if (-not (Test-Path -LiteralPath (Join-Path $frontend "packages\dsh-web-ui-all\lib\index.js"))) {
        throw "未找到内置 frontend 构建产物；请先在 frontend/dsh-web-ui 执行 pnpm install 与 pnpm build"
    }

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $installedBinary) | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $installedFrontend) | Out-Null
    Copy-Item -LiteralPath $binary -Destination $installedBinary -Force
    $copyArgs = @($frontend, $installedFrontend, "/E", "/XD", "node_modules", ".pnpm-store", "/NFL", "/NDL", "/NJH", "/NJS", "/NP")
    robocopy @copyArgs | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "复制内置 frontend 失败：$LASTEXITCODE" }
    Write-Host "Local install: $installedBinary" -ForegroundColor Green

    Invoke-DeepSeekSmokeTest
    $env:DEEPSEEK_MODEL = $Model
    $env:DEEPSEEK_BASE_URL = $BaseUrl
    $process = Start-Process -FilePath $installedBinary -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    $port = $null
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($process.HasExited) { throw "桌面进程提前退出，退出码：$($process.ExitCode)" }
        if (Test-Path -LiteralPath $logPath) {
            $currentLines = @(Get-Content -LiteralPath $logPath -ErrorAction SilentlyContinue)
            $tail = ($currentLines | Select-Object -Skip $logLineCount | Select-Object -Last 24 | Out-String)
            if ($tail -match "ERR_PNPM_|failed in profile|DSH 服务启动超时") {
                throw "DSH 本地服务启动失败；已停止测试。详细日志：$logPath"
            }
        }
        $port = Find-LocalService
        if ($null -ne $port) { break }
        Start-Sleep -Seconds 2
    }
    if ($null -eq $port) { throw "桌面端启动超时：未发现 127.0.0.1:3080-3180 的 DSH 服务" }
    Write-Host "Desktop smoke test: OK (http://127.0.0.1:$port/)" -ForegroundColor Green
    Write-Host "Full local validation: PASS" -ForegroundColor Green
} finally {
    if ($null -ne $process -and -not $process.HasExited) {
        taskkill.exe /PID $process.Id /T /F | Out-Null
        $null = $process.WaitForExit(5000)
    }
    if ($null -eq $previousModel) { Remove-Item Env:DEEPSEEK_MODEL -ErrorAction SilentlyContinue } else { $env:DEEPSEEK_MODEL = $previousModel }
    if ($null -eq $previousBaseUrl) { Remove-Item Env:DEEPSEEK_BASE_URL -ErrorAction SilentlyContinue } else { $env:DEEPSEEK_BASE_URL = $previousBaseUrl }
}
