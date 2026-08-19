[CmdletBinding()]
param(
    [string]$Output = $(Join-Path (Split-Path -Parent $PSScriptRoot) "dsh-desktop-windows-x64.zip"),
    [string]$NodeVersion = "v24.19.0",
    [string]$Mirror = $env:DSH_DESKTOP_MIRROR,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$frontend = Join-Path $root "frontend\dsh-web-ui"
$temp = Join-Path ([IO.Path]::GetTempPath()) ("dsh-package-" + [guid]::NewGuid().ToString("N"))
$dist = Join-Path $temp "dist"

function Convert-ToMirrorUrl([string]$Url) {
    if ([string]::IsNullOrWhiteSpace($Mirror)) { return $Url }
    if ($Mirror.Contains("{url}")) { return $Mirror.Replace("{url}", $Url) }
    return ($Mirror.TrimEnd("/") + "/" + $Url)
}

function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program 执行失败，退出码：$LASTEXITCODE" }
}

function Download-NodeRuntime([string]$Destination) {
    $archiveName = "node-$NodeVersion-win-x64.zip"
    $archive = Join-Path $temp $archiveName
    $direct = "https://nodejs.org/dist/$NodeVersion/$archiveName"
    $downloaded = $false
    $urls = @((Convert-ToMirrorUrl $direct), $direct) | Select-Object -Unique
    foreach ($url in $urls) {
        try {
            Write-Host "下载 Node.js：$url"
            Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archive
            $downloaded = $true
            break
        } catch {
            Write-Warning "下载失败：$($_.Exception.Message)"
        }
    }
    if (-not $downloaded) { throw "无法下载 Node.js $NodeVersion" }

    $extract = Join-Path $temp "node-runtime"
    Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
    $nodeRoot = Join-Path $extract "node-$NodeVersion-win-x64"
    if (-not (Test-Path -LiteralPath (Join-Path $nodeRoot "node.exe"))) {
        throw "Node.js 压缩包内容异常"
    }
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-Item -Path (Join-Path $nodeRoot "*") -Destination $Destination -Recurse -Force
    Invoke-Checked (Join-Path $Destination "corepack.cmd") @("enable", "--install-directory", $Destination)
    $actual = (& (Join-Path $Destination "node.exe") --version).Trim()
    if ($actual -ne $NodeVersion) { throw "Node.js 版本校验失败：$actual" }
    if (-not (Test-Path -LiteralPath (Join-Path $Destination "pnpm.cmd"))) {
        throw "内置 Corepack 未生成 pnpm.cmd"
    }
}

New-Item -ItemType Directory -Force -Path $temp | Out-Null
try {
    if (-not $SkipBuild) {
        if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
            throw "未找到 pnpm；请先启用 Node.js 24 LTS 的 Corepack"
        }
        Invoke-Checked "corepack" @("enable")
        Invoke-Checked "pnpm" @("--dir", $frontend, "install", "--frozen-lockfile", "--config.minimumReleaseAge=0")
        Invoke-Checked "pnpm" @("--dir", $frontend, "build")
        python -c "import PIL" 2>$null
        if ($LASTEXITCODE -ne 0) {
            Invoke-Checked "python" @("-m", "pip", "install", "pillow")
        }
        Invoke-Checked "python" @("scripts/generate-icons.py")
        Invoke-Checked "cargo" @("build", "--release", "--locked", "--manifest-path", (Join-Path $root "Cargo.toml"))
    }

    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    Copy-Item -LiteralPath (Join-Path $root "target\release\dsh-desktop.exe") -Destination (Join-Path $dist "dsh-desktop.exe") -Force
    $copyArgs = @($frontend, (Join-Path $dist "frontend\dsh-web-ui"), "/E", "/XD", "node_modules", ".pnpm-store", "/NFL", "/NDL", "/NJH", "/NJS", "/NP")
    robocopy @copyArgs | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "复制内置 frontend 失败：$LASTEXITCODE" }
    Download-NodeRuntime (Join-Path $dist "runtime\node")

    $outputPath = [IO.Path]::GetFullPath($Output)
    if (Test-Path -LiteralPath $outputPath) { Remove-Item -LiteralPath $outputPath -Force }
    $items = @(Get-ChildItem -LiteralPath $dist | Select-Object -ExpandProperty Name)
    & tar.exe -a -c -f $outputPath -C $dist @items
    if ($LASTEXITCODE -ne 0) { throw "Windows 安装包压缩失败" }
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $outputPath).Hash
    Write-Host "Windows x64 安装包已生成：$outputPath" -ForegroundColor Green
    Write-Host "SHA256：$hash"
} finally {
    if (Test-Path -LiteralPath $temp) {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
