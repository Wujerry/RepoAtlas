[CmdletBinding()]
param(
    [switch]$Force,
    [switch]$Release
)

$ErrorActionPreference = 'Stop'

if (-not $Force) {
    throw 'This operation clears RepoAtlas app data and replaces the marked showcase fixtures. Re-run with -Force when you intend to prepare the local demo.'
}

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The local RepoAtlas showcase seeder is supported by this script on Windows only.'
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd('\')
$showcaseRoot = [System.IO.Path]::GetFullPath('C:\RepoAtlas Showcase').TrimEnd('\')
$markerName = '.repoatlas-demo-marker'
$markerLine = 'RepoAtlas demo showcase marker v1'
$fixtureSlugs = @(
    'atlas-dashboard',
    'signal-console',
    'insight-notebooks',
    'harbor-api',
    'care-portal',
    'field-kit',
    'ops-playbook',
    'pulse-mobile'
)

function Get-ExactAppDataPath {
    if ([string]::IsNullOrWhiteSpace($env:APPDATA)) {
        throw 'APPDATA is not available; refusing to guess an application-data location.'
    }

    $appDataRoot = [System.IO.Path]::GetFullPath($env:APPDATA).TrimEnd('\')
    $root = [System.IO.Path]::GetFullPath((Join-Path $appDataRoot 'io.repoatlas.desktop')).TrimEnd('\')
    $database = [System.IO.Path]::GetFullPath((Join-Path $root 'repoatlas.sqlite'))
    if (-not [System.String]::Equals([System.IO.Path]::GetFileName($root), 'io.repoatlas.desktop', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected RepoAtlas app-data directory: $root"
    }

    [pscustomobject]@{
        Root = $root
        Database = $database
    }
}

function Assert-RepoAtlasClosed {
    $knownNames = @('RepoAtlas', 'repoatlas', 'repoatlas-mcp', 'repoatlas-mcp.exe')
    $running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
        $knownNames -contains $_.ProcessName
    })
    if ($running.Count -gt 0) {
        $names = ($running | ForEach-Object { $_.ProcessName } | Sort-Object -Unique) -join ', '
        throw "RepoAtlas is still running ($names). Close the desktop app and MCP process before preparing the showcase."
    }
}

function Write-Utf8File {
    param(
        [Parameter(Mandatory)][string]$RelativePath,
        [Parameter(Mandatory)][AllowEmptyString()][string]$Content
    )

    $target = [System.IO.Path]::GetFullPath((Join-Path $script:showcaseRoot $RelativePath))
    $rootWithSlash = "$script:showcaseRoot\"
    if (-not $target.StartsWith($rootWithSlash, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to write outside the showcase root: $RelativePath"
    }
    $parent = Split-Path -Parent $target
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [System.IO.File]::WriteAllText($target, $Content, [System.Text.UTF8Encoding]::new($false))
}

function Invoke-LocalGit {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string[]]$Arguments
    )

    $output = & git -C $Path @Arguments 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed in $Path`n$output"
    }
}

function Initialize-GitFixture {
    param(
        [Parameter(Mandatory)][string]$Slug,
        [switch]$Dirty
    )

    $path = Join-Path $script:showcaseRoot $Slug
    Invoke-LocalGit -Path $path -Arguments @('init')
    Invoke-LocalGit -Path $path -Arguments @('branch', '-M', 'main')
    Invoke-LocalGit -Path $path -Arguments @('config', 'user.name', 'RepoAtlas Demo')
    Invoke-LocalGit -Path $path -Arguments @('config', 'user.email', 'demo@repoatlas.local')
    Invoke-LocalGit -Path $path -Arguments @('config', 'commit.gpgsign', 'false')
    Invoke-LocalGit -Path $path -Arguments @('add', '.')
    Invoke-LocalGit -Path $path -Arguments @('commit', '-m', 'Create showcase fixture')

    if ($Dirty) {
        Write-Utf8File -RelativePath "$Slug/src/status.ts" -Content @'
export const status = "ready-for-review";
export const note = "Unstaged status update for the Git workspace demo.";
'@
        Write-Utf8File -RelativePath "$Slug/release-notes.md" -Content @'
# Release notes

The staged note is ready for review before the next mobile handoff.
'@
        Invoke-LocalGit -Path $path -Arguments @('add', 'release-notes.md')
    }
}

function Assert-MarkedFixtureSafe {
    if (-not (Test-Path -LiteralPath $script:showcaseRoot -PathType Container)) {
        return
    }

    $rootItem = Get-Item -LiteralPath $script:showcaseRoot -Force
    if ($rootItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
        throw "Refusing to use a reparse-point showcase root: $script:showcaseRoot"
    }

    $markerPath = Join-Path $script:showcaseRoot $script:markerName
    if (-not (Test-Path -LiteralPath $markerPath -PathType Leaf)) {
        throw "Showcase root exists without the RepoAtlas marker; refusing to remove anything: $script:showcaseRoot"
    }
    $markerText = [System.IO.File]::ReadAllText($markerPath)
    if (-not ($markerText -split "`r?`n" | Where-Object { $_.Trim() -eq $script:markerLine })) {
        throw "Showcase marker does not identify RepoAtlas demo data; refusing to remove anything: $script:showcaseRoot"
    }

    foreach ($slug in $script:fixtureSlugs) {
        $fixturePath = [System.IO.Path]::GetFullPath((Join-Path $script:showcaseRoot $slug)).TrimEnd('\')
        $rootWithSlash = "$script:showcaseRoot\"
        if (-not $fixturePath.StartsWith($rootWithSlash, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove a fixture outside the showcase root: $fixturePath"
        }
        if (Test-Path -LiteralPath $fixturePath) {
            $item = Get-Item -LiteralPath $fixturePath -Force
            if ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
                throw "Refusing to remove a reparse-point fixture: $fixturePath"
            }
        }
    }
}

function Remove-MarkedFixture {
    Assert-MarkedFixtureSafe
    if (-not (Test-Path -LiteralPath $script:showcaseRoot -PathType Container)) {
        New-Item -ItemType Directory -Path $script:showcaseRoot -Force | Out-Null
        return
    }

    foreach ($slug in $script:fixtureSlugs) {
        $fixturePath = [System.IO.Path]::GetFullPath((Join-Path $script:showcaseRoot $slug)).TrimEnd('\')
        if (Test-Path -LiteralPath $fixturePath) {
            Remove-Item -LiteralPath $fixturePath -Recurse -Force
        }
    }
}

function Write-ShowcaseFixtures {
    Write-Utf8File -RelativePath 'atlas-dashboard/package.json' -Content @'
{
  "name": "atlas-dashboard",
  "private": true,
  "packageManager": "pnpm@9.15.0",
  "engines": { "node": ">=20" },
  "scripts": {
    "dev": "vite",
    "test": "vitest run",
    "build": "vite build",
    "lint": "eslint ."
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@vitejs/plugin-react": "^4.3.0",
    "vite": "^7.0.0",
    "vitest": "^3.0.0"
  }
}
'@
    Write-Utf8File -RelativePath 'atlas-dashboard/pnpm-lock.yaml' -Content @'
lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      react:
        specifier: ^19.0.0
        version: 19.0.0
      react-dom:
        specifier: ^19.0.0
        version: 19.0.0
'@
    Write-Utf8File -RelativePath 'atlas-dashboard/README.md' -Content @'
# Atlas Dashboard

Atlas Dashboard is a small React and Vite interface for making a growing local project library easy to scan.

## Demo loop

Use the **Start preview**, **Run checks**, and **Production build** tasks to walk through a complete discovery-to-action loop.
'@
    Write-Utf8File -RelativePath 'atlas-dashboard/src/App.tsx' -Content @'
export function App() {
  return <main aria-label="Atlas Dashboard">Local projects, clearly indexed.</main>;
}
'@
    Write-Utf8File -RelativePath 'atlas-dashboard/src/lib/search.test.ts' -Content @'
import { describe, expect, it } from "vitest";

describe("project search", () => {
  it("keeps the local index predictable", () => {
    expect("Atlas Dashboard".toLowerCase()).toContain("atlas");
  });
});
'@
    Write-Utf8File -RelativePath 'atlas-dashboard/.gitignore' -Content @'
node_modules/
dist/
.env
'@

    Write-Utf8File -RelativePath 'signal-console/Cargo.toml' -Content @'
[package]
name = "signal-console"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
tauri = { version = "2" }
'@
    Write-Utf8File -RelativePath 'signal-console/Cargo.lock' -Content @'
version = 4

[[package]]
name = "signal-console"
version = "0.1.0"
dependencies = [
 "serde",
 "tauri",
]
'@
    Write-Utf8File -RelativePath 'signal-console/tauri.conf.json' -Content @'
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Signal Console",
  "identifier": "local.repoatlas.signal-console"
}
'@
    Write-Utf8File -RelativePath 'signal-console/src/main.rs' -Content @'
fn main() {
    println!("Signal Console demo");
}
'@
    Write-Utf8File -RelativePath 'signal-console/README.md' -Content @'
# Signal Console

Signal Console is a compact Rust and Tauri desktop shell for keeping operator context close to local work.

The fixture is intentionally self-contained and has no remote repository.
'@

    Write-Utf8File -RelativePath 'insight-notebooks/pyproject.toml' -Content @'
[project]
name = "insight-notebooks"
version = "0.1.0"
description = "Reviewable local experiments"
requires-python = ">=4.0"
dependencies = [
  "fastapi>=0.115",
  "uvicorn>=0.30",
]

[tool.uv]
package = false
'@
    Write-Utf8File -RelativePath 'insight-notebooks/uv.lock' -Content @'
version = 1
revision = 1

requires-python = ">=4.0"
'@
    Write-Utf8File -RelativePath 'insight-notebooks/main.py' -Content @'
from fastapi import FastAPI

app = FastAPI(title="Insight Notebooks")

@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ready"}
'@
    Write-Utf8File -RelativePath 'insight-notebooks/tests/test_health.py' -Content @'
def test_health_contract() -> None:
    assert {"status": "ready"}["status"] == "ready"
'@
    Write-Utf8File -RelativePath 'insight-notebooks/README.md' -Content @'
# Insight Notebooks

Insight Notebooks turns local observations into small, reviewable experiments with a FastAPI endpoint and a lightweight uv workflow.

The declared Python 4 runtime is deliberately unavailable on a normal machine so the Environment panel can demonstrate a visible mismatch.
'@

    Write-Utf8File -RelativePath 'harbor-api/go.mod' -Content @'
module example.local/harbor-api

go 1.22

require github.com/gin-gonic/gin v1.10.0
'@
    Write-Utf8File -RelativePath 'harbor-api/go.sum' -Content @'
github.com/gin-gonic/gin v1.10.0 h1:showcase-only
'@
    Write-Utf8File -RelativePath 'harbor-api/main.go' -Content @'
package main

import "github.com/gin-gonic/gin"

func main() {
    router := gin.New()
    router.GET("/health", func(context *gin.Context) { context.JSON(200, gin.H{"status": "ready"}) })
    _ = router.Run(":8080")
}
'@
    Write-Utf8File -RelativePath 'harbor-api/README.md' -Content @'
# Harbor API

Harbor API is a focused Go service with explicit health checks and a dependable test command.

It is a local-only fixture; the module path is fictional and has no remote checkout.
'@

    Write-Utf8File -RelativePath 'care-portal/pom.xml' -Content @'
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <groupId>local.repoatlas</groupId>
  <artifactId>care-portal</artifactId>
  <version>0.1.0</version>
  <parent>
    <groupId>org.springframework.boot</groupId>
    <artifactId>spring-boot-starter-parent</artifactId>
    <version>3.4.0</version>
  </parent>
  <properties><java.version>21</java.version></properties>
  <dependencies>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-web</artifactId>
    </dependency>
  </dependencies>
</project>
'@
    Write-Utf8File -RelativePath 'care-portal/src/main/java/local/repoatlas/care/CarePortalApplication.java' -Content @'
package local.repoatlas.care;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

@SpringBootApplication
public class CarePortalApplication {
    public static void main(String[] args) {
        SpringApplication.run(CarePortalApplication.class, args);
    }
}
'@
    Write-Utf8File -RelativePath 'care-portal/README.md' -Content @'
# Care Portal

Care Portal is a Spring Boot service prototype for coordinating care-team workflows.

The Java 21 declaration is visible in the detected environment facts without requiring a network build.
'@

    Write-Utf8File -RelativePath 'field-kit/pubspec.yaml' -Content @'
name: field_kit
description: An offline-first companion for field notes.
publish_to: none
environment:
  sdk: '>=3.5.0 <4.0.0'
dependencies:
  flutter:
    sdk: flutter
  collection: ^1.19.0
'@
    Write-Utf8File -RelativePath 'field-kit/lib/main.dart' -Content @'
import 'package:flutter/material.dart';

void main() => runApp(const FieldKitApp());

class FieldKitApp extends StatelessWidget {
  const FieldKitApp({super.key});

  @override
  Widget build(BuildContext context) => const MaterialApp(home: Text('Field Kit'));
}
'@
    Write-Utf8File -RelativePath 'field-kit/README.md' -Content @'
# Field Kit

Field Kit is a Flutter companion app for field notes, checklists, and offline-first handoffs.
'@

    Write-Utf8File -RelativePath 'ops-playbook/Makefile' -Content @'
.PHONY: preview check

preview:
	python -m http.server 4173

check:
	@echo "playbook links look good"
'@
    Write-Utf8File -RelativePath 'ops-playbook/README.md' -Content @'
# Ops Playbook

Ops Playbook is a versioned set of operational notes with a tiny local preview task and no version-control metadata.

Use it to demonstrate archived records and a project without a Git repository.
'@
    Write-Utf8File -RelativePath 'ops-playbook/docs/runbook.md' -Content @'
# Local handoff

1. Open the project overview.
2. Review the latest note.
3. Run the preview task when a local reader is useful.
'@

    Write-Utf8File -RelativePath 'pulse-mobile/package.json' -Content @'
{
  "name": "pulse-mobile",
  "private": true,
  "packageManager": "pnpm@9.15.0",
  "scripts": {
    "dev": "vite",
    "test": "vitest run",
    "build": "vite build"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-native": "^0.76.0"
  },
  "devDependencies": {
    "typescript": "^5.7.0",
    "vite": "^7.0.0"
  }
}
'@
    Write-Utf8File -RelativePath 'pulse-mobile/src/status.ts' -Content @'
export const status = "ready";
export const note = "Baseline status before the Git workspace demo.";
'@
    Write-Utf8File -RelativePath 'pulse-mobile/README.md' -Content @'
# Pulse Mobile

Pulse Mobile is a compact TypeScript product surface with a healthy task set and a deliberately visible Git change.

The fixture keeps one staged release note and one unstaged status update so the Git view can explain both states.
'@
    Write-Utf8File -RelativePath 'pulse-mobile/.gitignore' -Content @'
node_modules/
dist/
'@
}

$locations = Get-ExactAppDataPath
Assert-RepoAtlasClosed
Assert-MarkedFixtureSafe

# The child cleanup script repeats the process and exact-path checks immediately
# before deleting only the three database files and direct task-log files.
$clearScript = Join-Path $PSScriptRoot 'clear-repoatlas-data.ps1'
& $clearScript -Force
if ($LASTEXITCODE -ne 0) {
    throw "RepoAtlas data cleanup failed with exit code $LASTEXITCODE"
}

Remove-MarkedFixture
New-Item -ItemType Directory -Path $script:showcaseRoot -Force | Out-Null
Write-Utf8File -RelativePath $markerName -Content "$markerLine`nThis directory is generated by scripts/seed-showcase.ps1 and contains fictional data only.`n"
Write-ShowcaseFixtures

foreach ($slug in @('atlas-dashboard', 'signal-console', 'insight-notebooks', 'harbor-api', 'care-portal', 'field-kit', 'pulse-mobile')) {
    Initialize-GitFixture -Slug $slug -Dirty:($slug -eq 'pulse-mobile')
}

$cargoArguments = @('run', '--quiet', '-p', 'repoatlas-core', '--bin', 'repoatlas-demo', '--', '--db', $locations.Database, '--showcase', $script:showcaseRoot)
if ($Release) {
    $cargoArguments = @('run', '--quiet', '--release', '-p', 'repoatlas-core', '--bin', 'repoatlas-demo', '--', '--db', $locations.Database, '--showcase', $script:showcaseRoot)
}

Push-Location $repoRoot
try {
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "repoatlas-demo failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

Write-Host "Showcase prepared at $script:showcaseRoot"
Write-Host 'Fictional fixture projects: 8 (7 Git, 1 without VCS; pulse-mobile has staged and unstaged changes).'
Write-Host 'RepoAtlas database was rebuilt without touching any existing source directory.'
