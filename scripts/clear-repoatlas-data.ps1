[CmdletBinding()]
param(
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

if (-not $Force) {
    throw 'This operation removes the local RepoAtlas database, WAL/SHM files, and task logs. Re-run with -Force when you intend to clear only RepoAtlas app data.'
}

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'RepoAtlas local app-data cleanup is supported by this script on Windows only.'
}

function Get-ExactAppDataPath {
    if ([string]::IsNullOrWhiteSpace($env:APPDATA)) {
        throw 'APPDATA is not available; refusing to guess an application-data location.'
    }

    $appDataRoot = [System.IO.Path]::GetFullPath($env:APPDATA).TrimEnd('\')
    $expectedRoot = [System.IO.Path]::GetFullPath((Join-Path $appDataRoot 'io.repoatlas.desktop')).TrimEnd('\')
    $expectedDb = [System.IO.Path]::GetFullPath((Join-Path $expectedRoot 'repoatlas.sqlite'))

    if (-not [System.String]::Equals(
            [System.IO.Path]::GetFileName($expectedRoot),
            'io.repoatlas.desktop',
            [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected RepoAtlas app-data directory: $expectedRoot"
    }

    [pscustomobject]@{
        Root = $expectedRoot
        Database = $expectedDb
        Wal = "$expectedDb-wal"
        Shm = "$expectedDb-shm"
        TaskLogs = [System.IO.Path]::GetFullPath((Join-Path $expectedRoot 'task-logs')).TrimEnd('\')
    }
}

function Assert-RepoAtlasClosed {
    $knownNames = @('RepoAtlas', 'repoatlas', 'repoatlas-mcp', 'repoatlas-mcp.exe')
    $running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
        $knownNames -contains $_.ProcessName
    })

    if ($running.Count -gt 0) {
        $names = ($running | ForEach-Object { $_.ProcessName } | Sort-Object -Unique) -join ', '
        throw "RepoAtlas is still running ($names). Close the desktop app and MCP process before clearing its database."
    }
}

function Remove-ExactFile {
    param([Parameter(Mandatory)][string]$Path)

    if (Test-Path -LiteralPath $Path -PathType Leaf) {
        $fullPath = [System.IO.Path]::GetFullPath($Path)
        if (-not [System.String]::Equals($fullPath, $Path, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove a path that is not exact: $Path"
        }
        Remove-Item -LiteralPath $Path -Force
        Write-Host "Removed $Path"
    }
}

$locations = Get-ExactAppDataPath
Assert-RepoAtlasClosed

# These values are constructed from APPDATA and the fixed identifier; no
# user-supplied path can be passed to this script. Keep the check explicit so a
# malformed environment cannot turn cleanup into a broad recursive delete.
$expectedRootName = [System.IO.Path]::GetFileName($locations.Root)
if (-not [System.String]::Equals($expectedRootName, 'io.repoatlas.desktop', [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean an unexpected application-data directory: $($locations.Root)"
}

Remove-ExactFile -Path $locations.Database
Remove-ExactFile -Path $locations.Wal
Remove-ExactFile -Path $locations.Shm

if (Test-Path -LiteralPath $locations.TaskLogs -PathType Container) {
    $taskLogItem = Get-Item -LiteralPath $locations.TaskLogs -Force
    if ($taskLogItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
        throw "Refusing to traverse a reparse-point task-log directory: $($locations.TaskLogs)"
    }

    # Core writes task logs as direct files under this owned directory. Reject
    # unexpected subdirectories instead of recursively deleting unknown data.
    $children = @(Get-ChildItem -LiteralPath $locations.TaskLogs -Force)
    foreach ($child in $children) {
        if ($child.PSIsContainer -or ($child.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
            throw "Unexpected item under RepoAtlas task logs; refusing to delete it: $($child.FullName)"
        }
        $childPath = [System.IO.Path]::GetFullPath($child.FullName)
        $taskLogRootWithSlash = "$($locations.TaskLogs)\"
        if (-not $childPath.StartsWith($taskLogRootWithSlash, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to delete a task-log path outside RepoAtlas app data: $childPath"
        }
        Remove-Item -LiteralPath $child.FullName -Force
        Write-Host "Removed $childPath"
    }
}

Write-Host "RepoAtlas app data cleared under $($locations.Root). No project directories were touched."
