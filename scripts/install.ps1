[CmdletBinding()]
param(
    [string]$Version = "latest",
    [string]$Mirror = $env:DSH_DESKTOP_MIRROR,
    [string]$InstallDir = $(if ($env:DSH_DESKTOP_INSTALL_DIR) { $env:DSH_DESKTOP_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "DSH Desktop" }),
    [switch]$FromSource
)

$ErrorActionPreference = "Stop"
$repo = if ($env:DSH_DESKTOP_REPO) { $env:DSH_DESKTOP_REPO } else { "neko233-com/dsh-desktop" }
$asset = "dsh-desktop-windows-x64.zip"
$temp = Join-Path ([IO.Path]::GetTempPath()) ("dsh-desktop-" + [guid]::NewGuid().ToString("N"))

function Convert-ToMirrorUrl([string]$Url) {
    if ([string]::IsNullOrWhiteSpace($Mirror)) { return $Url }
    if ($Mirror.Contains("{url}")) { return $Mirror.Replace("{url}", $Url) }
    return ($Mirror.TrimEnd("/") + "/" + $Url)
}

function Get-AssetUrlList {
    if ($env:DSH_DESKTOP_DOWNLOAD_URL) { return @($env:DSH_DESKTOP_DOWNLOAD_URL) }
    $direct = if ($Version -eq "latest") {
        "https://github.com/$repo/releases/latest/download/$asset"
    } else {
        "https://github.com/$repo/releases/download/$Version/$asset"
    }
    $urls = @()
    if ($Mirror) { $urls += Convert-ToMirrorUrl $direct }
    $urls += $direct
    return $urls | Select-Object -Unique
}

function Install-FromSource {
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "未找到 git，无法源码构建" }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "未找到 Rust/cargo；请安装 rustup 后重试" }
    $source = Join-Path $temp "source"
    $cloneUrl = Convert-ToMirrorUrl "https://github.com/$repo.git"
    git clone --depth 1 $cloneUrl $source
    cargo build --release --locked --manifest-path (Join-Path $source "Cargo.toml")
    $binary = Join-Path $source "target\release\dsh-desktop.exe"
    if (-not (Test-Path -LiteralPath $binary)) { throw "源码构建未生成 dsh-desktop.exe" }
    New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "bin") | Out-Null
    Copy-Item -LiteralPath $binary -Destination (Join-Path $InstallDir "bin\dsh-desktop.exe") -Force
}

New-Item -ItemType Directory -Force -Path $temp | Out-Null
try {
    $archive = Join-Path $temp $asset
    if (-not $FromSource) {
        $downloaded = $false
        foreach ($url in (Get-AssetUrlList)) {
            try {
                Write-Host "下载 $url"
                Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archive
                $downloaded = $true
                break
            } catch {
                Write-Warning "下载失败：$($_.Exception.Message)"
            }
        }
        if ($downloaded) {
            $extract = Join-Path $temp "extract"
            Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
            $binary = Get-ChildItem -LiteralPath $extract -Filter "dsh-desktop.exe" -Recurse -File | Select-Object -First 1
            if (-not $binary) { throw "发布包缺少 dsh-desktop.exe" }
            New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "bin") | Out-Null
            Copy-Item -LiteralPath $binary.FullName -Destination (Join-Path $InstallDir "bin\dsh-desktop.exe") -Force
        } else {
            throw "没有可用发布包。请改用 -FromSource，或设置 DSH_DESKTOP_MIRROR。"
        }
    } else {
        Install-FromSource
    }

    $binDir = Join-Path $InstallDir "bin"
    if ($env:DSH_NPM_REGISTRY) {
        Set-Content -LiteralPath (Join-Path $InstallDir "npm-registry") -Value $env:DSH_NPM_REGISTRY -NoNewline
    }
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathItems = @($userPath -split ";" | Where-Object { $_ })
    if ($pathItems -notcontains $binDir) {
        [Environment]::SetEnvironmentVariable("Path", (($pathItems + $binDir) -join ";"), "User")
    }
    $env:Path = "$binDir;$env:Path"
    Write-Host "DSH Desktop 已安装：$binDir\dsh-desktop.exe" -ForegroundColor Green
    if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        Write-Warning "未检测到 Node.js 22+。首次启动 DSH 前请安装 Node.js，或设置 DSH_DESKTOP_DSH_BIN。"
    }
    Write-Host "启动：dsh-desktop"
} finally {
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    $resolvedTemp = [IO.Path]::GetFullPath($temp)
    if ($resolvedTemp.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $resolvedTemp)) {
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
