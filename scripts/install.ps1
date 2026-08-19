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

function Install-BundledNodeRuntime([string]$Destination) {
    $version = "v24.19.0"
    $nodeAsset = "node-$version-win-x64.zip"
    $nodeArchive = Join-Path $temp $nodeAsset
    $nodeUrls = @()
    $nodeDirect = "https://nodejs.org/dist/$version/$nodeAsset"
    if ($Mirror) { $nodeUrls += Convert-ToMirrorUrl $nodeDirect }
    $nodeUrls += $nodeDirect
    $downloaded = $false
    foreach ($url in ($nodeUrls | Select-Object -Unique)) {
        try {
            Write-Host "下载 Node.js 运行时 $url"
            Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $nodeArchive
            $downloaded = $true
            break
        } catch {
            Write-Warning "Node.js 下载失败：$($_.Exception.Message)"
        }
    }
    if (-not $downloaded) { throw "无法下载内置 Node.js 运行时。请设置 DSH_DESKTOP_MIRROR 后重试。" }
    $nodeExtract = Join-Path $temp "node-runtime"
    Expand-Archive -LiteralPath $nodeArchive -DestinationPath $nodeExtract -Force
    $nodeRoot = Join-Path $nodeExtract "node-$version-win-x64"
    if (Test-Path -LiteralPath $Destination) {
        Remove-Item -LiteralPath $Destination -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-Item -Path (Join-Path $nodeRoot "*") -Destination $Destination -Recurse -Force
    $node = Join-Path $Destination "node.exe"
    if ((& $node --version).Trim() -ne $version) { throw "内置 Node.js 版本校验失败" }
}

function Install-FromSource {
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "未找到 git，无法源码构建" }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "未找到 Rust/cargo；请安装 rustup 后重试" }
    if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) { throw "未找到 pnpm；请先安装 Node.js 24+ 并启用 Corepack" }
    $source = Join-Path $temp "source"
    $cloneUrl = Convert-ToMirrorUrl "https://github.com/$repo.git"
    git clone --depth 1 $cloneUrl $source
    $frontend = Join-Path $source "frontend\dsh-web-ui"
    if ($env:DSH_NPM_REGISTRY) {
        $env:NPM_CONFIG_REGISTRY = $env:DSH_NPM_REGISTRY
        $env:npm_config_registry = $env:DSH_NPM_REGISTRY
    }
    pnpm --dir $frontend install --frozen-lockfile --config.minimumReleaseAge=0
    pnpm --dir $frontend build
    cargo build --release --locked --manifest-path (Join-Path $source "Cargo.toml")
    $binary = Join-Path $source "target\release\dsh-desktop.exe"
    if (-not (Test-Path -LiteralPath $binary)) { throw "源码构建未生成 dsh-desktop.exe" }
    New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "bin") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "frontend") | Out-Null
    Install-BundledNodeRuntime (Join-Path $InstallDir "runtime\node")
    Copy-Item -LiteralPath $binary -Destination (Join-Path $InstallDir "bin\dsh-desktop.exe") -Force
    $copyArgs = @($frontend, (Join-Path $InstallDir "frontend\dsh-web-ui"), "/E", "/XD", "node_modules", ".pnpm-store", "/NFL", "/NDL", "/NJH", "/NJS", "/NP")
    robocopy @copyArgs | Out-Null
    if ($LASTEXITCODE -gt 7) { throw "复制内置 frontend 失败：$LASTEXITCODE" }
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
            $frontend = Join-Path $extract "frontend"
            if (-not (Test-Path -LiteralPath (Join-Path $frontend "dsh-web-ui"))) { throw "发布包缺少内置 frontend/dsh-web-ui" }
            if (-not (Test-Path -LiteralPath (Join-Path $extract "runtime\node\node.exe"))) { throw "发布包缺少内置 Node.js 运行时" }
            New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "bin") | Out-Null
            New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "frontend") | Out-Null
            New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir "runtime") | Out-Null
            Copy-Item -LiteralPath $binary.FullName -Destination (Join-Path $InstallDir "bin\dsh-desktop.exe") -Force
            $copyArgs = @((Join-Path $frontend "dsh-web-ui"), (Join-Path $InstallDir "frontend\dsh-web-ui"), "/E", "/XD", "node_modules", ".pnpm-store", "/NFL", "/NDL", "/NJH", "/NJS", "/NP")
            robocopy @copyArgs | Out-Null
            if ($LASTEXITCODE -gt 7) { throw "复制内置 frontend 失败：$LASTEXITCODE" }
            $runtimeDestination = Join-Path $InstallDir "runtime\node"
            if (Test-Path -LiteralPath $runtimeDestination) {
                Remove-Item -LiteralPath $runtimeDestination -Recurse -Force
            }
            Copy-Item -LiteralPath (Join-Path $extract "runtime\node") -Destination $runtimeDestination -Recurse -Force
            if ((& (Join-Path $runtimeDestination "node.exe") --version).Trim() -ne "v24.19.0") { throw "发布包 Node.js 版本校验失败" }
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
    Write-Host "已内置 Node.js 运行时；首次启动会自动准备并更新官方 DeepSeek Harness。" -ForegroundColor Green
    Write-Host "启动：dsh-desktop"
} finally {
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    $resolvedTemp = [IO.Path]::GetFullPath($temp)
    if ($resolvedTemp.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $resolvedTemp)) {
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
