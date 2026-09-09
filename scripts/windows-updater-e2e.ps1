param(
  [string]$Target = "x86_64-pc-windows-msvc",
  [int]$Port = 38491
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Step([string]$Message) {
  Write-Host ""
  Write-Host "=== $Message ===" -ForegroundColor Cyan
}

function Set-RepoAtlasVersion {
  param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$UpdaterPubkey,
    [Parameter(Mandatory = $true)][string]$Endpoint,
    [switch]$AutoInstallOnStartup
  )

  $packagePath = Join-Path $PWD "package.json"
  $package = Get-Content -Raw -LiteralPath $packagePath | ConvertFrom-Json
  $package.version = $Version
  $package | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $packagePath -Encoding utf8

  $cargoPath = Join-Path $PWD "Cargo.toml"
  $cargo = Get-Content -Raw -LiteralPath $cargoPath
  $cargo = [regex]::Replace(
    $cargo,
    '(?ms)(\[workspace\.package\].*?^version\s*=\s*")[^"]+(")',
    ('$1' + $Version + '$2'),
    1
  )
  Set-Content -LiteralPath $cargoPath -Value $cargo -Encoding utf8 -NoNewline

  $configPath = Join-Path $PWD "src-tauri\tauri.conf.json"
  $config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
  $config.version = $Version
  $config.plugins.updater.pubkey = $UpdaterPubkey.Trim()
  $config.plugins.updater.endpoints = @($Endpoint)
  if ($null -eq $config.plugins.updater.PSObject.Properties["dangerousInsecureTransportProtocol"]) {
    $config.plugins.updater | Add-Member -MemberType NoteProperty -Name dangerousInsecureTransportProtocol -Value $true
  } else {
    $config.plugins.updater.dangerousInsecureTransportProtocol = $true
  }
  $config | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $configPath -Encoding utf8

  $appPath = Join-Path $PWD "src\App.tsx"
  git checkout -- $appPath
  if ($AutoInstallOnStartup) {
    $app = Get-Content -Raw -LiteralPath $appPath
    $pattern = 'void updaterService\.checkForUpdates\(\);\s*\r?\n\s*\}, \[bootError, booting\]\);'
    $replacement = @'
void (async () => {
      const checked = await updaterService.checkForUpdates();
      if (checked.status === "available") {
        const downloaded = await updaterService.downloadUpdate();
        if (downloaded.status === "ready") {
          await updaterService.installUpdate();
        }
      }
    })();
  }, [bootError, booting]);
'@
    $patched = [regex]::Replace($app, $pattern, $replacement, 1)
    if ($patched -eq $app) {
      throw "Failed to patch startup updater flow for the E2E test."
    }
    Set-Content -LiteralPath $appPath -Value $patched -Encoding utf8 -NoNewline
  }
}

function Get-NsisInstaller {
  param([Parameter(Mandatory = $true)][string]$Version)
  $bundleDir = Join-Path $PWD "target\$Target\release\bundle\nsis"
  if (-not (Test-Path -LiteralPath $bundleDir)) {
    throw "NSIS bundle directory not found: $bundleDir"
  }
  $installer = Get-ChildItem -LiteralPath $bundleDir -Filter "*$Version*-setup.exe" -File |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
  if (-not $installer) {
    $installer = Get-ChildItem -LiteralPath $bundleDir -Filter "*-setup.exe" -File |
      Sort-Object LastWriteTimeUtc -Descending |
      Select-Object -First 1
  }
  if (-not $installer) {
    throw "No NSIS setup executable found below $bundleDir"
  }
  $sigPath = "$($installer.FullName).sig"
  if (-not (Test-Path -LiteralPath $sigPath)) {
    throw "Updater signature not found: $sigPath"
  }
  return [pscustomobject]@{
    Installer = $installer
    SignaturePath = $sigPath
  }
}

function Get-RepoAtlasInstallEntry {
  $roots = @(
    "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
    "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*",
    "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*"
  )
  foreach ($root in $roots) {
    $entry = Get-ItemProperty -Path $root -ErrorAction SilentlyContinue |
      Where-Object { $_.DisplayName -eq "RepoAtlas" } |
      Select-Object -First 1
    if ($entry) { return $entry }
  }
  return $null
}

function Wait-ForInstallVersion {
  param(
    [Parameter(Mandatory = $true)][string]$Version,
    [int]$TimeoutSeconds = 180
  )
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    $entry = Get-RepoAtlasInstallEntry
    if ($entry -and [string]$entry.DisplayVersion -eq $Version) {
      return $entry
    }
    Start-Sleep -Seconds 2
  } while ((Get-Date) -lt $deadline)
  $current = Get-RepoAtlasInstallEntry
  $currentVersion = if ($current) { [string]$current.DisplayVersion } else { "<not installed>" }
  throw "Timed out waiting for RepoAtlas $Version. Current installed version: $currentVersion"
}

function Get-MainExecutable {
  param([Parameter(Mandatory = $true)]$InstallEntry)
  $location = [string]$InstallEntry.InstallLocation
  $location = $location.Trim('"')
  if ([string]::IsNullOrWhiteSpace($location) -and $InstallEntry.UninstallString) {
    $uninstallPath = ([string]$InstallEntry.UninstallString).Trim().Trim('"')
    $location = Split-Path -Parent $uninstallPath
  }
  if ([string]::IsNullOrWhiteSpace($location) -or -not (Test-Path -LiteralPath $location)) {
    throw "Could not resolve RepoAtlas install directory."
  }
  $exe = Get-ChildItem -LiteralPath $location -Filter "*.exe" -File |
    Where-Object { $_.Name -notmatch '^(uninstall|repoatlas-mcp)\.exe$' } |
    Select-Object -First 1
  if (-not $exe) {
    throw "Could not find RepoAtlas desktop executable in $location"
  }
  return $exe
}

$serverProcess = $null
try {
  Write-Step "Install dependencies"
  pnpm install --frozen-lockfile

  Write-Step "Generate disposable updater signing key"
  $keyDir = Join-Path $env:RUNNER_TEMP "repoatlas-updater-e2e-key"
  New-Item -ItemType Directory -Force -Path $keyDir | Out-Null
  $keyPath = Join-Path $keyDir "updater.key"
  $password = "repoatlas-e2e"
  pnpm tauri signer generate -w $keyPath -p $password
  $pubPath = "$keyPath.pub"
  if (-not (Test-Path -LiteralPath $pubPath)) {
    throw "Tauri signer did not create expected public key: $pubPath"
  }
  $pubkey = Get-Content -Raw -LiteralPath $pubPath
  $env:TAURI_SIGNING_PRIVATE_KEY = $keyPath
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $password

  $serverDir = Join-Path $env:RUNNER_TEMP "repoatlas-updater-e2e-server"
  New-Item -ItemType Directory -Force -Path $serverDir | Out-Null
  $endpoint = "http://127.0.0.1:$Port/latest.json"

  Write-Step "Build signed test update 0.1.1"
  Set-RepoAtlasVersion -Version "0.1.1" -UpdaterPubkey $pubkey -Endpoint $endpoint
  pnpm tauri build --target $Target --bundles nsis
  $newBundle = Get-NsisInstaller -Version "0.1.1"
  $newInstallerName = $newBundle.Installer.Name
  Copy-Item -LiteralPath $newBundle.Installer.FullName -Destination (Join-Path $serverDir $newInstallerName) -Force
  Copy-Item -LiteralPath $newBundle.SignaturePath -Destination (Join-Path $serverDir "$newInstallerName.sig") -Force

  $signature = (Get-Content -Raw -LiteralPath $newBundle.SignaturePath).Trim()
  $encodedInstaller = [uri]::EscapeDataString($newInstallerName)
  $manifest = [ordered]@{
    version = "0.1.1"
    notes = "RepoAtlas Windows updater E2E"
    pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
      "windows-x86_64" = [ordered]@{
        signature = $signature
        url = "http://127.0.0.1:$Port/$encodedInstaller"
      }
    }
  }
  $manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $serverDir "latest.json") -Encoding utf8

  Write-Step "Build old test app 0.1.0 with automatic E2E driver"
  Set-RepoAtlasVersion -Version "0.1.0" -UpdaterPubkey $pubkey -Endpoint $endpoint -AutoInstallOnStartup
  pnpm tauri build --target $Target --bundles nsis
  $oldBundle = Get-NsisInstaller -Version "0.1.0"

  Write-Step "Install old version silently"
  $existing = Get-RepoAtlasInstallEntry
  if ($existing -and $existing.UninstallString) {
    $uninstall = ([string]$existing.UninstallString).Trim().Trim('"')
    if (Test-Path -LiteralPath $uninstall) {
      Start-Process -FilePath $uninstall -ArgumentList "/S" -Wait
    }
  }
  Start-Process -FilePath $oldBundle.Installer.FullName -ArgumentList "/S" -Wait
  $oldEntry = Wait-ForInstallVersion -Version "0.1.0" -TimeoutSeconds 90
  $oldExe = Get-MainExecutable -InstallEntry $oldEntry
  Write-Host "Installed old version at $($oldExe.FullName)"

  Write-Step "Start local updater endpoint"
  $serverLog = Join-Path $env:RUNNER_TEMP "repoatlas-updater-e2e-http.log"
  $serverErr = Join-Path $env:RUNNER_TEMP "repoatlas-updater-e2e-http.err.log"
  $serverArgs = @("-m", "http.server", "$Port", "--bind", "127.0.0.1")
  $serverProcess = Start-Process -FilePath "python" -ArgumentList $serverArgs -WorkingDirectory $serverDir -RedirectStandardOutput $serverLog -RedirectStandardError $serverErr -PassThru
  $ready = $false
  for ($i = 0; $i -lt 30; $i++) {
    try {
      $response = Invoke-WebRequest -Uri $endpoint -UseBasicParsing -TimeoutSec 2
      if ($response.StatusCode -eq 200) {
        $ready = $true
        break
      }
    } catch {}
    Start-Sleep -Seconds 1
  }
  if (-not $ready) {
    throw "Local updater endpoint did not become ready."
  }

  Write-Step "Launch 0.1.0 and perform check -> download -> install"
  Start-Process -FilePath $oldExe.FullName | Out-Null
  $updatedEntry = Wait-ForInstallVersion -Version "0.1.1" -TimeoutSeconds 180
  $updatedExe = Get-MainExecutable -InstallEntry $updatedEntry
  Write-Host "Updated installation found at $($updatedExe.FullName)"

  Write-Step "Verify updated app launches"
  $updatedProcess = Start-Process -FilePath $updatedExe.FullName -PassThru
  Start-Sleep -Seconds 8
  if ($updatedProcess.HasExited) {
    throw "Updated RepoAtlas process exited unexpectedly with code $($updatedProcess.ExitCode)."
  }
  Stop-Process -Id $updatedProcess.Id -Force -ErrorAction SilentlyContinue

  Write-Step "Verify updater endpoint was actually used"
  Start-Sleep -Seconds 2
  if (Test-Path -LiteralPath $serverLog) {
    Get-Content -LiteralPath $serverLog | Write-Host
  }
  if (-not (Test-Path -LiteralPath $serverErr)) {
    throw "HTTP server log was not created."
  }
  Get-Content -LiteralPath $serverErr | Write-Host
  $httpText = Get-Content -Raw -LiteralPath $serverErr
  if ($httpText -notmatch 'GET /latest\.json') {
    throw "Updater did not request latest.json."
  }
  if ($httpText -notmatch [regex]::Escape("GET /$newInstallerName")) {
    throw "Updater did not request the Windows installer."
  }

  Write-Step "Windows updater E2E passed"
  Write-Host "PASS: RepoAtlas upgraded from 0.1.0 to 0.1.1 through the real Tauri updater path."
}
finally {
  if ($serverProcess -and -not $serverProcess.HasExited) {
    Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
  }
  $entry = Get-RepoAtlasInstallEntry
  if ($entry -and $entry.UninstallString) {
    $uninstall = ([string]$entry.UninstallString).Trim().Trim('"')
    if (Test-Path -LiteralPath $uninstall) {
      try { Start-Process -FilePath $uninstall -ArgumentList "/S" -Wait } catch {}
    }
  }
  git reset --hard HEAD | Out-Null
}
