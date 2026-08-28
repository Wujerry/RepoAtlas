use crate::models::{DependencySnapshot, DetectedFact, TaskDefinition};
use crate::paths;
use chrono::{SecondsFormat, Utc};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;
use uuid::Uuid;

// Detection runs on directories selected by the user (or an MCP client), so
// malformed or generated manifests must not turn a refresh into an unbounded
// memory read.  The preview layer has its own limit; this smaller cap keeps
// the scanner cheap while still covering normal manifests.
const MAX_DETECT_TEXT_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone)]
pub struct Detection {
    pub name: String,
    pub vcs_kind: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub package_managers: Vec<String>,
    pub facts: Vec<DetectedFact>,
    pub tasks: Vec<TaskDefinition>,
    pub dependencies: Option<DependencySnapshot>,
    pub readme_path: Option<String>,
    pub readme_excerpt: Option<String>,
    pub search_blob: String,
    pub source_mtime: Option<String>,
}

pub fn is_skip_dir(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "node_modules"
            | "target"
            | "dist"
            | "build"
            | "out"
            | ".git"
            | ".svn"
            | ".hg"
            | ".idea"
            | ".vscode"
            | ".next"
            | ".nuxt"
            | ".turbo"
            | ".cache"
            | "coverage"
            | "__pycache__"
            | ".venv"
            | "venv"
            | "vendor"
            | "bin"
            | "obj"
            | ".gradle"
            | ".dart_tool"
    )
}

pub fn is_project_root(path: &Path) -> bool {
    // Scanning uses this predicate as a stop condition. A Project may live
    // below a Git checkout, but an arbitrary directory below that checkout
    // must not become a project merely because Git metadata exists above it.
    // `detect` still resolves the repository root for manually registered
    // nested Projects and for directories which contain their own manifest.
    path.join(".git").exists() || path.join(".svn").exists() || !collect_manifests(path).is_empty()
}

pub fn detect(path: &Path) -> Detection {
    let vcs_kind = detect_vcs(path).unwrap_or("none").to_string();
    let manifests = collect_manifests(path);
    let mut languages = Vec::new();
    let mut frameworks = Vec::new();
    let mut package_managers = Vec::new();
    let mut facts = Vec::new();
    let mut tasks = Vec::new();
    let mut declared = Vec::new();
    let mut lockfile = None;
    let mut name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled".into());

    for manifest in &manifests {
        push_fact(&mut facts, "manifest", manifest, 0.95, manifest);
        match manifest.as_str() {
            "package.json" => {
                languages.push("JavaScript".into());
                push_fact(&mut facts, "language", "JavaScript", 0.95, manifest);
                let package_manager = detect_js_package_manager(path);
                package_managers.push(package_manager.clone());
                push_fact(
                    &mut facts,
                    "packageManager",
                    &package_manager,
                    0.95,
                    manifest,
                );
                if let Some(pkg) = read_json(path.join("package.json")) {
                    if let Some(pkg_name) = pkg.get("name").and_then(Value::as_str) {
                        if !pkg_name.is_empty() {
                            name = pkg_name.to_string();
                        }
                    }
                    if has_package(&pkg, "react") {
                        frameworks.push("React".into());
                        push_fact(&mut facts, "framework", "React", 0.9, manifest);
                    }
                    if has_package(&pkg, "vite") {
                        frameworks.push("Vite".into());
                        push_fact(&mut facts, "framework", "Vite", 0.9, manifest);
                    }
                    if has_package(&pkg, "@tauri-apps/cli") || has_package(&pkg, "@tauri-apps/api")
                    {
                        frameworks.push("Tauri".into());
                        push_fact(&mut facts, "framework", "Tauri", 0.9, manifest);
                    }
                    if has_package(&pkg, "vue") {
                        frameworks.push("Vue".into());
                        push_fact(&mut facts, "framework", "Vue", 0.9, manifest);
                    }
                    if has_package(&pkg, "next") {
                        frameworks.push("Next.js".into());
                        push_fact(&mut facts, "framework", "Next.js", 0.9, manifest);
                    }
                    if has_package(&pkg, "nuxt") {
                        frameworks.push("Nuxt".into());
                        push_fact(&mut facts, "framework", "Nuxt", 0.9, manifest);
                    }
                    if has_package(&pkg, "svelte") || has_package(&pkg, "@sveltejs/kit") {
                        frameworks.push("Svelte".into());
                        push_fact(&mut facts, "framework", "Svelte", 0.9, manifest);
                    }
                    if has_package(&pkg, "express") {
                        frameworks.push("Express".into());
                        push_fact(&mut facts, "framework", "Express", 0.85, manifest);
                    }
                    if has_package(&pkg, "nestjs") || has_package(&pkg, "@nestjs/core") {
                        frameworks.push("NestJS".into());
                        push_fact(&mut facts, "framework", "NestJS", 0.9, manifest);
                    }
                    if has_package(&pkg, "electron") {
                        frameworks.push("Electron".into());
                        push_fact(&mut facts, "framework", "Electron", 0.9, manifest);
                    }
                    if has_package(&pkg, "angular") || has_package(&pkg, "@angular/core") {
                        frameworks.push("Angular".into());
                        push_fact(&mut facts, "framework", "Angular", 0.9, manifest);
                    }
                    if let Some(scripts) = pkg.get("scripts").and_then(Value::as_object) {
                        let mut names: Vec<_> = scripts.keys().cloned().collect();
                        names.sort();
                        for script in names {
                            let kind = infer_script_kind(&script);
                            tasks.push(named_task(
                                &script,
                                kind,
                                &package_manager,
                                vec!["run".into(), script.clone()],
                            ));
                        }
                    }
                    collect_deps(&mut declared, pkg.get("dependencies"));
                }
                detect_js_runtime(path, &mut facts);
                if path.join("pnpm-lock.yaml").exists() {
                    lockfile = Some("pnpm-lock.yaml".into());
                } else if path.join("yarn.lock").exists() {
                    lockfile = Some("yarn.lock".into());
                } else if path.join("package-lock.json").exists() {
                    lockfile = Some("package-lock.json".into());
                }
            }
            "Cargo.toml" => {
                languages.push("Rust".into());
                push_fact(&mut facts, "language", "Rust", 0.95, manifest);
                package_managers.push("cargo".into());
                push_fact(&mut facts, "packageManager", "cargo", 0.95, manifest);
                let cargo_text = read_text(&path.join("Cargo.toml")).unwrap_or_default();
                if let Ok(value) = cargo_text.parse::<toml::Value>() {
                    if let Some(pkg_name) = value
                        .get("package")
                        .and_then(|pkg| pkg.get("name"))
                        .and_then(|n| n.as_str())
                    {
                        name = pkg_name.to_string();
                    }
                    if let Some(rust_version) = value
                        .get("package")
                        .and_then(|pkg| pkg.get("rust-version"))
                        .and_then(|item| item.as_str())
                    {
                        push_runtime(
                            &mut facts,
                            "rust",
                            rust_version,
                            "Cargo.toml#package.rust-version",
                        );
                    }
                    if cargo_has_dep(&value, "axum")
                        || cargo_has_dep(&value, "actix-web")
                        || cargo_has_dep(&value, "rocket")
                    {
                        frameworks.push("Rust Web".into());
                        push_fact(&mut facts, "framework", "Rust Web", 0.85, manifest);
                    }
                    if cargo_has_dep(&value, "bevy") {
                        frameworks.push("Bevy".into());
                        push_fact(&mut facts, "framework", "Bevy", 0.9, manifest);
                    }
                    if cargo_has_dep(&value, "dioxus") {
                        frameworks.push("Dioxus".into());
                        push_fact(&mut facts, "framework", "Dioxus", 0.9, manifest);
                    }
                    if cargo_has_dep(&value, "leptos") {
                        frameworks.push("Leptos".into());
                        push_fact(&mut facts, "framework", "Leptos", 0.9, manifest);
                    }
                }
                detect_rust_toolchain(path, &mut facts);
                if path.join("src-tauri").exists()
                    || path.join("tauri.conf.json").exists()
                    || cargo_text.contains("tauri")
                {
                    frameworks.push("Tauri".into());
                    push_fact(&mut facts, "framework", "Tauri", 0.9, manifest);
                    tasks.push(described_task(
                        "dev",
                        "tauri-dev",
                        "Start the Tauri development app",
                        "cargo",
                        vec!["tauri", "dev"],
                    ));
                    tasks.push(described_task(
                        "package",
                        "tauri-build",
                        "Build the Tauri desktop bundle",
                        "cargo",
                        vec!["tauri", "build"],
                    ));
                }
                if path.join("src/main.rs").is_file()
                    || cargo_text.contains("[[bin]]")
                    || cargo_text.contains("default-run")
                {
                    tasks.push(task("dev", "cargo", vec!["run"]));
                }
                tasks.extend([
                    task("test", "cargo", vec!["test"]),
                    task("build", "cargo", vec!["build"]),
                    task("package", "cargo", vec!["build", "--release"]),
                    described_task(
                        "check",
                        "clippy",
                        "Run Clippy on the Rust workspace",
                        "cargo",
                        vec!["clippy", "--all-targets", "--all-features"],
                    ),
                    described_task(
                        "check",
                        "fmt",
                        "Check Rust formatting",
                        "cargo",
                        vec!["fmt", "--all", "--", "--check"],
                    ),
                ]);
                if path.join("Cargo.lock").exists() {
                    lockfile = Some("Cargo.lock".into());
                }
            }
            "pyproject.toml" | "requirements.txt" | "Pipfile" | "uv.lock" | "environment.yml"
            | "environment.yaml" => {
                detect_python(
                    path,
                    manifest,
                    &name,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                    &mut lockfile,
                );
            }
            "go.mod" => {
                detect_go(
                    path,
                    manifest,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                    &mut lockfile,
                );
            }
            "pom.xml" => {
                detect_maven(
                    path,
                    manifest,
                    &mut name,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                    &mut lockfile,
                );
            }
            "build.gradle" | "build.gradle.kts" | "settings.gradle" | "settings.gradle.kts" => {
                detect_gradle(
                    path,
                    manifest,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                    &mut lockfile,
                );
            }
            "composer.json" => {
                detect_php(
                    path,
                    manifest,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                    &mut lockfile,
                );
            }
            "Gemfile" => {
                detect_ruby(
                    path,
                    manifest,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                    &mut lockfile,
                );
            }
            "mix.exs" => {
                detect_elixir(
                    path,
                    manifest,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                );
            }
            "pubspec.yaml" => {
                languages.push("Dart".into());
                push_fact(&mut facts, "language", "Dart", 0.95, manifest);
                package_managers.push("pub".into());
                push_fact(&mut facts, "packageManager", "pub", 0.95, manifest);
                let pubspec = read_text(&path.join("pubspec.yaml")).unwrap_or_default();
                if let Some(sdk) = yaml_scalar(&pubspec, "sdk") {
                    push_runtime(&mut facts, "dart", &sdk, "pubspec.yaml#environment.sdk");
                }
                if pubspec.contains("flutter:") {
                    frameworks.push("Flutter".into());
                    push_fact(&mut facts, "framework", "Flutter", 0.9, manifest);
                    tasks.extend([
                        task("dev", "flutter", vec!["run"]),
                        task("test", "flutter", vec!["test"]),
                        task("build", "flutter", vec!["build", "apk"]),
                    ]);
                } else {
                    tasks.extend([
                        task("run", "dart", vec!["run"]),
                        task("test", "dart", vec!["test"]),
                    ]);
                }
            }
            "CMakeLists.txt" | "meson.build" | "conanfile.txt" | "conanfile.py" | "vcpkg.json" => {
                detect_native(
                    path,
                    manifest,
                    &mut languages,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                );
            }
            "Makefile" | "makefile" => {
                tasks.extend([
                    described_task(
                        "build",
                        "make",
                        "Build via Makefile",
                        "make",
                        Vec::<&str>::new(),
                    ),
                    described_task(
                        "test",
                        "make-test",
                        "Run Makefile tests",
                        "make",
                        vec!["test"],
                    ),
                ]);
            }
            "Package.swift" | "Podfile" => {
                detect_apple(
                    path,
                    manifest,
                    &mut languages,
                    &mut frameworks,
                    &mut package_managers,
                    &mut facts,
                    &mut tasks,
                );
            }
            "build.zig" => {
                languages.push("Zig".into());
                push_fact(&mut facts, "language", "Zig", 0.95, manifest);
                package_managers.push("zig".into());
                push_fact(&mut facts, "packageManager", "zig", 0.95, manifest);
                tasks.extend([
                    task("run", "zig", vec!["build", "run"]),
                    task("test", "zig", vec!["build", "test"]),
                    task("build", "zig", vec!["build"]),
                ]);
            }
            "deno.json" | "deno.jsonc" => {
                languages.push("TypeScript".into());
                push_fact(&mut facts, "language", "TypeScript", 0.9, manifest);
                package_managers.push("deno".into());
                push_fact(&mut facts, "packageManager", "deno", 0.95, manifest);
                tasks.extend([
                    task("dev", "deno", vec!["task", "dev"]),
                    task("test", "deno", vec!["test"]),
                    task("run", "deno", vec!["run", "-A", "main.ts"]),
                ]);
            }
            "bun.lock" | "bun.lockb" => {
                languages.push("JavaScript".into());
                push_fact(&mut facts, "language", "JavaScript", 0.8, manifest);
                package_managers.push("bun".into());
                push_fact(&mut facts, "packageManager", "bun", 0.95, manifest);
            }
            "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => {
                push_fact(&mut facts, "container", "docker", 0.7, manifest);
                if manifest.starts_with("docker-compose") {
                    tasks.push(described_task(
                        "dev",
                        "compose",
                        "Start Docker Compose services",
                        "docker",
                        vec!["compose", "up"],
                    ));
                }
            }
            "go.sum" => {
                languages.push("Go".into());
                push_fact(&mut facts, "language", "Go", 0.8, manifest);
            }
            "tauri.conf.json" => {
                frameworks.push("Tauri".into());
                push_fact(&mut facts, "framework", "Tauri", 0.9, manifest);
            }
            _ => {
                if manifest.ends_with(".csproj")
                    || manifest.ends_with(".fsproj")
                    || manifest.ends_with(".vbproj")
                    || manifest.ends_with(".sln")
                {
                    detect_dotnet(
                        path,
                        manifest,
                        &mut languages,
                        &mut frameworks,
                        &mut package_managers,
                        &mut facts,
                        &mut tasks,
                    );
                } else if manifest.ends_with(".xcodeproj") {
                    detect_apple(
                        path,
                        manifest,
                        &mut languages,
                        &mut frameworks,
                        &mut package_managers,
                        &mut facts,
                        &mut tasks,
                    );
                }
            }
        }
    }

    if path.join("tsconfig.json").exists() && !languages.iter().any(|l| l == "TypeScript") {
        languages.push("TypeScript".into());
        push_fact(&mut facts, "language", "TypeScript", 0.95, "tsconfig.json");
    }

    if let Some(lockfile) = lockfile.as_deref() {
        push_fact(&mut facts, "lockfile", lockfile, 0.95, lockfile);
    }
    detect_version_files(path, &mut facts);
    detect_project_icon(path, &mut facts);
    if vcs_kind == "git" {
        if let Some(remote) = crate::git::origin_url(path) {
            push_fact(&mut facts, "lineage", &remote, 0.9, "git remote origin");
        }
    }

    let readme_path = find_readme(path);
    let readme_excerpt = readme_path
        .as_ref()
        .and_then(|file| read_text(&path.join(file)).map(|text| excerpt(&text, 2400)));

    uniq(&mut languages);
    uniq(&mut frameworks);
    uniq(&mut package_managers);
    uniq_tasks(&mut tasks);
    sort_facts(&mut facts);

    let source_mtime = newest_mtime(path, &manifests, readme_path.as_deref());
    let search_blob = [
        name.clone(),
        paths::path_to_string(path),
        languages.join(" "),
        frameworks.join(" "),
        package_managers.join(" "),
        vcs_kind.clone(),
        readme_excerpt.clone().unwrap_or_default(),
    ]
    .join("\n");

    Detection {
        name,
        vcs_kind,
        languages,
        frameworks,
        package_managers: package_managers.clone(),
        facts,
        tasks,
        dependencies: if declared.is_empty() && lockfile.is_none() {
            None
        } else {
            Some(DependencySnapshot {
                package_manager: package_managers.first().cloned(),
                declared,
                lockfile,
                observed_at: now(),
            })
        },
        readme_path,
        readme_excerpt,
        search_blob,
        source_mtime,
    }
}

fn detect_vcs(path: &Path) -> Option<&'static str> {
    if crate::git::repository_root(path).is_some() {
        Some("git")
    } else if path.join(".svn").exists() {
        Some("svn")
    } else {
        None
    }
}

fn collect_manifests(path: &Path) -> Vec<String> {
    const NAMES: &[&str] = &[
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
        "composer.json",
        "Gemfile",
        "mix.exs",
        "pubspec.yaml",
        "CMakeLists.txt",
        "Pipfile",
        "go.sum",
        "uv.lock",
        "environment.yml",
        "environment.yaml",
        "Dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
        "tauri.conf.json",
        "Makefile",
        "makefile",
        "Package.swift",
        "Podfile",
        "meson.build",
        "conanfile.txt",
        "conanfile.py",
        "vcpkg.json",
        "build.zig",
        "deno.json",
        "deno.jsonc",
        "bun.lock",
        "bun.lockb",
    ];
    let mut found = Vec::new();
    for name in NAMES {
        if path.join(name).is_file() {
            found.push((*name).to_string());
        }
    }
    if let Ok(entries) = fs::read_dir(path) {
        let mut extra = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let lower = name.to_ascii_lowercase();
                (lower.ends_with(".csproj")
                    || lower.ends_with(".fsproj")
                    || lower.ends_with(".vbproj")
                    || lower.ends_with(".sln")
                    || lower.ends_with(".xcodeproj"))
                .then_some(name)
            })
            .collect::<Vec<_>>();
        extra.sort();
        for name in extra {
            found.push(name);
        }
    }
    found
}

// These detectors write to the same small detection result and intentionally
// take each output collection explicitly; keep the call contract readable.
#[allow(clippy::too_many_arguments)]
fn detect_maven(
    path: &Path,
    manifest: &str,
    name: &mut String,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
    _lockfile: &mut Option<String>,
) {
    let pom = read_text(&path.join("pom.xml")).unwrap_or_default();
    languages.push("Java".into());
    push_fact(facts, "language", "Java", 0.95, manifest);
    if pom.contains("<artifactId>kotlin-maven-plugin</artifactId>") || pom.contains("kotlin-stdlib")
    {
        languages.push("Kotlin".into());
        push_fact(facts, "language", "Kotlin", 0.9, manifest);
    }
    package_managers.push("maven".into());
    push_fact(facts, "packageManager", "maven", 0.95, manifest);
    let maven = if cfg!(windows) && path.join("mvnw.cmd").is_file() {
        "mvnw.cmd"
    } else if path.join("mvnw").is_file() {
        "./mvnw"
    } else {
        "mvn"
    };
    if let Some(artifact) = xml_tag_value(&pom, "artifactId") {
        if !artifact.is_empty() {
            *name = artifact;
        }
    }
    if pom.contains("spring-boot") {
        frameworks.push("Spring Boot".into());
        push_fact(facts, "framework", "Spring Boot", 0.95, manifest);
        tasks.push(described_task(
            "dev",
            "spring-boot",
            "Start the Spring Boot application",
            maven,
            vec!["spring-boot:run"],
        ));
    }
    if pom.contains("quarkus-maven-plugin") || pom.contains("quarkus") {
        frameworks.push("Quarkus".into());
        push_fact(facts, "framework", "Quarkus", 0.9, manifest);
        tasks.push(described_task(
            "dev",
            "quarkus-dev",
            "Start Quarkus in development mode",
            maven,
            vec!["quarkus:dev"],
        ));
    }
    if pom.contains("micronaut") {
        frameworks.push("Micronaut".into());
        push_fact(facts, "framework", "Micronaut", 0.9, manifest);
    }
    tasks.extend([
        described_task(
            "dev",
            "compile",
            "Compile Maven sources",
            maven,
            vec!["-q", "compile"],
        ),
        task("test", maven, vec!["test"]),
        task("package", maven, vec!["-DskipTests", "package"]),
        described_task(
            "check",
            "verify",
            "Run Maven verification",
            maven,
            vec!["verify"],
        ),
    ]);
    if path.join("mvnw").exists() || path.join("mvnw.cmd").exists() {
        push_fact(facts, "tooling", "maven-wrapper", 0.9, manifest);
    }
    if let Some(java) = xml_tag_value(&pom, "maven.compiler.source")
        .or_else(|| xml_tag_value(&pom, "java.version"))
        .or_else(|| xml_tag_value(&pom, "release"))
    {
        push_runtime(facts, "java", &java, "pom.xml");
    }
}

#[allow(clippy::too_many_arguments)]
fn detect_gradle(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
    _lockfile: &mut Option<String>,
) {
    let gradle = [
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ]
    .into_iter()
    .filter_map(|file| read_text(&path.join(file)))
    .collect::<Vec<_>>()
    .join(
        "
",
    );
    let wrapper = if cfg!(windows) && path.join("gradlew.bat").exists() {
        "gradlew.bat"
    } else if path.join("gradlew").exists() {
        "./gradlew"
    } else {
        "gradle"
    };
    let android = gradle.contains("com.android.application")
        || gradle.contains("com.android.library")
        || path.join("android/app/build.gradle").exists()
        || path.join("app/src/main/AndroidManifest.xml").exists();
    let kotlin = gradle.contains("kotlin(")
        || gradle.contains("org.jetbrains.kotlin")
        || gradle.contains("kotlin-android")
        || manifest.ends_with(".kts");
    if android {
        languages.push("Java".into());
        push_fact(facts, "language", "Java", 0.8, manifest);
        languages.push("Kotlin".into());
        push_fact(facts, "language", "Kotlin", 0.9, manifest);
        frameworks.push("Android".into());
        push_fact(facts, "framework", "Android", 0.95, manifest);
    } else if kotlin {
        languages.push("Kotlin".into());
        push_fact(facts, "language", "Kotlin", 0.95, manifest);
        languages.push("Java".into());
        push_fact(facts, "language", "Java", 0.7, manifest);
    } else {
        languages.push("Java".into());
        push_fact(facts, "language", "Java", 0.9, manifest);
    }
    package_managers.push("gradle".into());
    push_fact(facts, "packageManager", "gradle", 0.95, manifest);
    if gradle.contains("org.springframework.boot") || gradle.contains("spring-boot") {
        frameworks.push("Spring Boot".into());
        push_fact(facts, "framework", "Spring Boot", 0.95, manifest);
        tasks.push(described_task(
            "dev",
            "bootRun",
            "Start the Spring Boot application",
            wrapper,
            vec!["bootRun"],
        ));
    }
    if gradle.contains("io.quarkus") {
        frameworks.push("Quarkus".into());
        push_fact(facts, "framework", "Quarkus", 0.9, manifest);
        tasks.push(described_task(
            "dev",
            "quarkusDev",
            "Start Quarkus in development mode",
            wrapper,
            vec!["quarkusDev"],
        ));
    }
    if gradle.contains("io.micronaut") {
        frameworks.push("Micronaut".into());
        push_fact(facts, "framework", "Micronaut", 0.9, manifest);
    }
    if android {
        tasks.extend([
            described_task(
                "dev",
                "installDebug",
                "Install the Android debug build",
                wrapper,
                vec!["installDebug"],
            ),
            described_task(
                "test",
                "connectedCheck",
                "Run Android unit and device tests",
                wrapper,
                vec!["testDebugUnitTest"],
            ),
            described_task(
                "build",
                "assembleDebug",
                "Assemble the Android debug APK",
                wrapper,
                vec!["assembleDebug"],
            ),
            described_task(
                "package",
                "assembleRelease",
                "Assemble the Android release APK",
                wrapper,
                vec!["assembleRelease"],
            ),
        ]);
    } else {
        tasks.extend([
            described_task(
                "dev",
                "run",
                "Compile Gradle sources",
                wrapper,
                vec!["classes"],
            ),
            task("test", wrapper, vec!["test"]),
            task("build", wrapper, vec!["build"]),
        ]);
    }
    if wrapper != "gradle" {
        push_fact(facts, "tooling", "gradle-wrapper", 0.9, wrapper);
    }
    if let Some(java) = gradle_java_version(&gradle) {
        push_runtime(facts, "java", &java, manifest);
    }
}

fn cargo_has_dep(value: &toml::Value, name: &str) -> bool {
    [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "workspace.dependencies",
    ]
    .iter()
    .any(|section| {
        section
            .split('.')
            .try_fold(value, |current, key| current.get(key))
            .and_then(|node| node.get(name))
            .is_some()
    })
}

fn xml_tag_value(source: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = source.find(&open)? + open.len();
    let end = source[start..].find(&close)? + start;
    Some(source[start..end].trim().to_string())
}

#[allow(clippy::too_many_arguments)]
fn detect_python(
    path: &Path,
    manifest: &str,
    fallback_name: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
    lockfile: &mut Option<String>,
) {
    languages.push("Python".into());
    push_fact(facts, "language", "Python", 0.95, manifest);
    let pyproject = read_text(&path.join("pyproject.toml")).unwrap_or_default();
    let requirements = read_text(&path.join("requirements.txt")).unwrap_or_default();
    let combined = format!("{pyproject}\n{requirements}");
    let manager = if path.join("uv.lock").exists() || pyproject.contains("[tool.uv]") {
        "uv"
    } else if path.join("poetry.lock").exists() || pyproject.contains("[tool.poetry]") {
        "poetry"
    } else if path.join("pdm.lock").exists() || pyproject.contains("[tool.pdm]") {
        "pdm"
    } else if path.join("Pipfile").exists() {
        "pipenv"
    } else if path.join("environment.yml").exists() || path.join("environment.yaml").exists() {
        "conda"
    } else {
        "pip"
    };
    package_managers.push(manager.into());
    push_fact(facts, "packageManager", manager, 0.9, manifest);
    detect_python_runtime(path, &pyproject, facts);
    if path.join("main.py").is_file() {
        python_task(
            tasks,
            manager,
            "run",
            "python",
            "Run main.py",
            "python",
            vec!["main.py"],
        );
    } else if path.join("__main__.py").is_file() {
        python_task(
            tasks,
            manager,
            "run",
            "python",
            "Run the Python package entry point",
            "python",
            vec!["."],
        );
    } else {
        let module = fallback_name.replace('-', "_");
        if path.join(&module).join("__main__.py").is_file() {
            python_task(
                tasks,
                manager,
                "run",
                "python",
                "Run the Python package entry point",
                "python",
                vec!["-m", &module],
            );
        }
    }
    python_task(
        tasks,
        manager,
        "test",
        "pytest",
        "Run pytest",
        "pytest",
        Vec::<&str>::new(),
    );
    if path.join("manage.py").is_file() {
        frameworks.push("Django".into());
        push_fact(facts, "framework", "Django", 0.9, manifest);
        python_task(
            tasks,
            manager,
            "dev",
            "runserver",
            "Start the Django development server",
            "python",
            vec!["manage.py", "runserver"],
        );
    }
    let fastapi_entry = if read_text(&path.join("main.py"))
        .is_some_and(|source| source.contains("FastAPI(") && source.contains("app"))
    {
        Some("main:app")
    } else if read_text(&path.join("app/main.py"))
        .is_some_and(|source| source.contains("FastAPI(") && source.contains("app"))
    {
        Some("app.main:app")
    } else {
        None
    };
    if combined.contains("fastapi") && fastapi_entry.is_some() {
        frameworks.push("FastAPI".into());
        push_fact(facts, "framework", "FastAPI", 0.9, manifest);
        python_task(
            tasks,
            manager,
            "dev",
            "uvicorn",
            "Start the FastAPI development server",
            "uvicorn",
            vec![fastapi_entry.unwrap_or("main:app"), "--reload"],
        );
    }
    if combined.contains("flask")
        && (path.join("app.py").is_file() || path.join("wsgi.py").is_file())
    {
        frameworks.push("Flask".into());
        push_fact(facts, "framework", "Flask", 0.9, manifest);
        python_task(
            tasks,
            manager,
            "dev",
            "flask",
            "Start the Flask development server",
            "flask",
            vec!["run"],
        );
    }
    if path.join("uv.lock").exists() {
        *lockfile = Some("uv.lock".into());
    } else if path.join("poetry.lock").exists() {
        *lockfile = Some("poetry.lock".into());
    } else if path.join("pdm.lock").exists() {
        *lockfile = Some("pdm.lock".into());
    }
}

#[allow(clippy::too_many_arguments)]
fn detect_go(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
    lockfile: &mut Option<String>,
) {
    languages.push("Go".into());
    push_fact(facts, "language", "Go", 0.95, manifest);
    package_managers.push("go".into());
    push_fact(facts, "packageManager", "go", 0.95, manifest);
    let gomod = read_text(&path.join("go.mod")).unwrap_or_default();
    if let Some(version) = go_directive(&gomod) {
        push_runtime(facts, "go", &version, "go.mod");
    }
    if gomod.contains("github.com/gin-gonic/gin") {
        frameworks.push("Gin".into());
        push_fact(facts, "framework", "Gin", 0.9, manifest);
    }
    if gomod.contains("github.com/labstack/echo") {
        frameworks.push("Echo".into());
        push_fact(facts, "framework", "Echo", 0.9, manifest);
    }
    if gomod.contains("github.com/gofiber/fiber") {
        frameworks.push("Fiber".into());
        push_fact(facts, "framework", "Fiber", 0.9, manifest);
    }
    if gomod.contains("google.golang.org/grpc") {
        frameworks.push("gRPC".into());
        push_fact(facts, "framework", "gRPC", 0.85, manifest);
    }
    tasks.extend([
        task("run", "go", vec!["run", "."]),
        task("test", "go", vec!["test", "./..."]),
        task("build", "go", vec!["build", "./..."]),
        described_task(
            "check",
            "vet",
            "Run Go static checks",
            "go",
            vec!["vet", "./..."],
        ),
        described_task(
            "check",
            "mod-tidy",
            "Tidy Go modules",
            "go",
            vec!["mod", "tidy"],
        ),
    ]);
    if path.join("go.sum").exists() {
        *lockfile = Some("go.sum".into());
    }
}

#[allow(clippy::too_many_arguments)]
fn detect_php(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
    lockfile: &mut Option<String>,
) {
    languages.push("PHP".into());
    push_fact(facts, "language", "PHP", 0.95, manifest);
    package_managers.push("composer".into());
    push_fact(facts, "packageManager", "composer", 0.95, manifest);
    let composer = read_text(&path.join("composer.json")).unwrap_or_default();
    if path.join("artisan").exists() || composer.contains("laravel/framework") {
        frameworks.push("Laravel".into());
        push_fact(facts, "framework", "Laravel", 0.95, manifest);
        tasks.push(described_task(
            "dev",
            "serve",
            "Start the Laravel development server",
            "php",
            vec!["artisan", "serve"],
        ));
        tasks.push(described_task(
            "test",
            "artisan-test",
            "Run Laravel tests",
            "php",
            vec!["artisan", "test"],
        ));
    }
    if composer.contains("symfony/framework-bundle") || path.join("bin/console").exists() {
        frameworks.push("Symfony".into());
        push_fact(facts, "framework", "Symfony", 0.9, manifest);
        tasks.push(described_task(
            "dev",
            "symfony-serve",
            "Start the Symfony local server",
            "symfony",
            vec!["serve"],
        ));
    }
    tasks.extend([
        task("test", "composer", vec!["test"]),
        task("build", "composer", vec!["install", "--no-dev"]),
    ]);
    if path.join("composer.lock").exists() {
        *lockfile = Some("composer.lock".into());
    }
}

#[allow(clippy::too_many_arguments)]
fn detect_ruby(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
    lockfile: &mut Option<String>,
) {
    languages.push("Ruby".into());
    push_fact(facts, "language", "Ruby", 0.95, manifest);
    package_managers.push("bundler".into());
    push_fact(facts, "packageManager", "bundler", 0.95, manifest);
    let gemfile = read_text(&path.join("Gemfile")).unwrap_or_default();
    if path.join("bin/rails").exists()
        || path.join("config/application.rb").exists()
        || gemfile.contains("rails")
    {
        frameworks.push("Rails".into());
        push_fact(facts, "framework", "Rails", 0.95, manifest);
        tasks.push(described_task(
            "dev",
            "rails-server",
            "Start the Rails development server",
            "bundle",
            vec!["exec", "rails", "server"],
        ));
        tasks.push(described_task(
            "test",
            "rails-test",
            "Run Rails tests",
            "bundle",
            vec!["exec", "rails", "test"],
        ));
    } else {
        tasks.push(task("test", "bundle", vec!["exec", "rake", "test"]));
    }
    if gemfile.contains("sinatra") {
        frameworks.push("Sinatra".into());
        push_fact(facts, "framework", "Sinatra", 0.85, manifest);
    }
    if path.join("Gemfile.lock").exists() {
        *lockfile = Some("Gemfile.lock".into());
    }
}

fn detect_elixir(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
) {
    languages.push("Elixir".into());
    push_fact(facts, "language", "Elixir", 0.95, manifest);
    package_managers.push("mix".into());
    push_fact(facts, "packageManager", "mix", 0.95, manifest);
    let mix = read_text(&path.join("mix.exs")).unwrap_or_default();
    if mix.contains(":phoenix")
        || path.join("lib").join("application.ex").exists() && mix.contains("Phoenix")
    {
        frameworks.push("Phoenix".into());
        push_fact(facts, "framework", "Phoenix", 0.9, manifest);
        tasks.push(described_task(
            "dev",
            "phx.server",
            "Start the Phoenix server",
            "mix",
            vec!["phx.server"],
        ));
    }
    tasks.extend([
        task("test", "mix", vec!["test"]),
        task("build", "mix", vec!["compile"]),
        described_task(
            "check",
            "format",
            "Check Elixir formatting",
            "mix",
            vec!["format", "--check-formatted"],
        ),
    ]);
}

fn detect_native(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
) {
    languages.push("C++".into());
    push_fact(facts, "language", "C++", 0.9, manifest);
    if path.join("CMakeLists.txt").exists() {
        package_managers.push("cmake".into());
        push_fact(facts, "packageManager", "cmake", 0.9, "CMakeLists.txt");
        tasks.extend([
            described_task(
                "build",
                "cmake-configure",
                "Configure the CMake build",
                "cmake",
                vec!["-S", ".", "-B", "build"],
            ),
            task("build", "cmake", vec!["--build", "build"]),
            task("test", "ctest", vec!["--test-dir", "build"]),
        ]);
    }
    if path.join("meson.build").exists() {
        package_managers.push("meson".into());
        push_fact(facts, "packageManager", "meson", 0.9, "meson.build");
        tasks.push(described_task(
            "build",
            "meson",
            "Build with Meson",
            "meson",
            vec!["compile", "-C", "build"],
        ));
    }
    if path.join("conanfile.txt").exists() || path.join("conanfile.py").exists() {
        package_managers.push("conan".into());
        push_fact(facts, "packageManager", "conan", 0.85, manifest);
    }
    if path.join("vcpkg.json").exists() {
        package_managers.push("vcpkg".into());
        push_fact(facts, "packageManager", "vcpkg", 0.85, manifest);
    }
}

fn detect_apple(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
) {
    languages.push("Swift".into());
    push_fact(facts, "language", "Swift", 0.95, manifest);
    if path.join("Package.swift").exists() {
        package_managers.push("spm".into());
        push_fact(facts, "packageManager", "spm", 0.95, "Package.swift");
        tasks.extend([
            task("test", "swift", vec!["test"]),
            task("build", "swift", vec!["build"]),
            described_task(
                "run",
                "swift-run",
                "Run the Swift package",
                "swift",
                vec!["run"],
            ),
        ]);
    }
    if path.join("Podfile").exists() {
        package_managers.push("cocoapods".into());
        push_fact(facts, "packageManager", "cocoapods", 0.9, "Podfile");
        frameworks.push("CocoaPods".into());
        push_fact(facts, "framework", "CocoaPods", 0.8, "Podfile");
    }
    if manifest.ends_with(".xcodeproj") || path.join("*.xcworkspace").exists() {
        frameworks.push("Xcode".into());
        push_fact(facts, "framework", "Xcode", 0.85, manifest);
    }
}

fn detect_dotnet(
    path: &Path,
    manifest: &str,
    languages: &mut Vec<String>,
    frameworks: &mut Vec<String>,
    package_managers: &mut Vec<String>,
    facts: &mut Vec<DetectedFact>,
    tasks: &mut Vec<TaskDefinition>,
) {
    let language = if manifest.ends_with(".fsproj") {
        "F#"
    } else if manifest.ends_with(".vbproj") {
        "Visual Basic"
    } else {
        "C#"
    };
    languages.push(language.into());
    push_fact(facts, "language", language, 0.95, manifest);
    package_managers.push("dotnet".into());
    push_fact(facts, "packageManager", "dotnet", 0.95, manifest);
    let project = read_text(&path.join(manifest)).unwrap_or_default();
    if let Some(tfm) = xml_tag_value(&project, "TargetFramework")
        .or_else(|| xml_tag_value(&project, "TargetFrameworks"))
    {
        push_runtime(facts, "dotnet", &tfm, manifest);
    }
    if project.contains("Microsoft.NET.Sdk.Web") || project.contains("Microsoft.AspNetCore") {
        frameworks.push("ASP.NET".into());
        push_fact(facts, "framework", "ASP.NET", 0.9, manifest);
    }
    if project.contains("Microsoft.NET.Sdk.BlazorWebAssembly") || project.contains("Blazor") {
        frameworks.push("Blazor".into());
        push_fact(facts, "framework", "Blazor", 0.9, manifest);
    }
    if project.contains("Microsoft.NET.Sdk.Maui") {
        frameworks.push("MAUI".into());
        push_fact(facts, "framework", "MAUI", 0.9, manifest);
    }
    tasks.extend([
        task("dev", "dotnet", vec!["watch", "run"]),
        task("test", "dotnet", vec!["test"]),
        task("build", "dotnet", vec!["build"]),
        described_task(
            "package",
            "publish",
            "Publish the .NET project",
            "dotnet",
            vec!["publish", "-c", "Release"],
        ),
    ]);
}

fn python_task(
    tasks: &mut Vec<TaskDefinition>,
    manager: &str,
    kind: &str,
    name: &str,
    description: &str,
    command: &str,
    argv: Vec<&str>,
) {
    match manager {
        "uv" => {
            let mut args = vec!["run", command];
            args.extend(argv);
            tasks.push(described_task(kind, name, description, "uv", args));
        }
        "poetry" => {
            let mut args = vec!["run", command];
            args.extend(argv);
            tasks.push(described_task(kind, name, description, "poetry", args));
        }
        "pdm" => {
            let mut args = vec!["run", command];
            args.extend(argv);
            tasks.push(described_task(kind, name, description, "pdm", args));
        }
        "pipenv" => {
            let mut args = vec!["run", command];
            args.extend(argv);
            tasks.push(described_task(kind, name, description, "pipenv", args));
        }
        _ => tasks.push(described_task(kind, name, description, command, argv)),
    }
}

fn detect_js_package_manager(path: &Path) -> String {
    if let Some(manager) = read_json(path.join("package.json"))
        .and_then(|package| package.get("packageManager")?.as_str().map(str::to_string))
        .and_then(|value| value.split('@').next().map(str::to_string))
        .filter(|value| matches!(value.as_str(), "npm" | "pnpm" | "yarn" | "bun"))
    {
        return manager;
    }

    // A package inside a Git monorepo commonly keeps its lockfile and package
    // manager declaration at the repository root. Walk only to that known
    // repository root so detection does not inherit an unrelated lockfile
    // from an arbitrary parent directory.
    let stop = crate::git::repository_root(path).unwrap_or_else(|| path.to_path_buf());
    let mut current = Some(path);
    while let Some(directory) = current {
        if directory.join("pnpm-lock.yaml").exists()
            || directory.join("pnpm-workspace.yaml").exists()
        {
            return "pnpm".into();
        }
        if directory.join("yarn.lock").exists() {
            return "yarn".into();
        }
        if directory.join("bun.lock").exists() || directory.join("bun.lockb").exists() {
            return "bun".into();
        }
        if directory == stop {
            break;
        }
        current = directory.parent();
    }
    "npm".into()
}

fn read_json(path: std::path::PathBuf) -> Option<Value> {
    read_text(&path).and_then(|text| serde_json::from_str(&text).ok())
}

fn has_package(package: &Value, name: &str) -> bool {
    ["dependencies", "devDependencies", "peerDependencies"]
        .iter()
        .any(|section| {
            package
                .get(section)
                .and_then(|value| value.get(name))
                .is_some()
        })
}

fn collect_deps(out: &mut Vec<String>, value: Option<&Value>) {
    if let Some(map) = value.and_then(Value::as_object) {
        for key in map.keys().take(24) {
            out.push(key.clone());
        }
    }
}

fn task(kind: &str, executable: &str, argv: Vec<&str>) -> TaskDefinition {
    described_task(
        kind,
        kind.to_uppercase(),
        default_task_description(kind, executable, &argv),
        executable,
        argv,
    )
}

fn described_task(
    kind: &str,
    name: impl Into<String>,
    description: impl Into<String>,
    executable: &str,
    argv: Vec<&str>,
) -> TaskDefinition {
    TaskDefinition {
        id: Uuid::new_v4().to_string(),
        kind: kind.into(),
        name: name.into(),
        description: Some(description.into()),
        executable: executable.into(),
        argv: argv.into_iter().map(str::to_string).collect(),
        cwd: None,
        inferred: true,
        shell_mode: false,
    }
}

fn fact(kind: &str, value: &str, confidence: f32, source: &str) -> DetectedFact {
    DetectedFact {
        kind: kind.into(),
        value: value.into(),
        confidence,
        source: source.into(),
    }
}

fn push_fact(
    facts: &mut Vec<DetectedFact>,
    kind: &str,
    value: &str,
    confidence: f32,
    source: &str,
) {
    if facts
        .iter()
        .any(|item| item.kind == kind && item.value == value)
    {
        return;
    }
    facts.push(fact(kind, value, confidence, source));
}

fn sort_facts(facts: &mut [DetectedFact]) {
    facts.sort_by(|left, right| {
        fact_rank(&left.kind)
            .cmp(&fact_rank(&right.kind))
            .then_with(|| left.value.cmp(&right.value))
            .then_with(|| left.source.cmp(&right.source))
    });
}

fn fact_rank(kind: &str) -> usize {
    match kind {
        "language" => 0,
        "framework" => 1,
        "packageManager" => 2,
        "manifest" => 3,
        "container" => 4,
        "lockfile" => 5,
        "runtime" => 6,
        "asset" => 7,
        "lineage" => 8,
        _ => 9,
    }
}

fn find_readme(path: &Path) -> Option<String> {
    const NAMES: &[&str] = &["README.md", "Readme.md", "readme.md", "README.MD", "README"];
    NAMES
        .iter()
        .find(|name| path.join(name).is_file())
        .map(|name| (*name).to_string())
}

fn excerpt(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        trimmed.chars().take(max).collect::<String>() + "..."
    }
}

fn uniq(values: &mut Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn uniq_tasks(tasks: &mut Vec<TaskDefinition>) {
    let mut seen = std::collections::BTreeSet::new();
    tasks.retain(|task| {
        seen.insert((
            task.kind.clone(),
            task.executable.clone(),
            task.argv.clone(),
        ))
    });
}

fn infer_script_kind(script: &str) -> &'static str {
    let lower = script.to_ascii_lowercase();
    if lower.contains("test")
        || lower.contains("spec")
        || lower.contains("lint")
        || lower.contains("check")
    {
        "test"
    } else if lower.contains("dev")
        || lower.contains("start")
        || lower.contains("serve")
        || lower.contains("watch")
    {
        "dev"
    } else if lower.contains("build") || lower.contains("compile") {
        "build"
    } else if lower.contains("package") || lower.contains("bundle") || lower.contains("dist") {
        "package"
    } else {
        "run"
    }
}

fn named_task(name: &str, kind: &str, executable: &str, argv: Vec<String>) -> TaskDefinition {
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    let mut task = described_task(
        kind,
        name,
        default_task_description(kind, executable, &argv_refs),
        executable,
        argv_refs,
    );
    task.argv = argv;
    task
}

fn default_task_description(kind: &str, executable: &str, argv: &[&str]) -> String {
    let command = std::iter::once(executable)
        .chain(argv.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    match kind {
        "dev" => format!("Start the local development workflow via `{command}`"),
        "test" => format!("Run the project test suite via `{command}`"),
        "build" => format!("Compile the project via `{command}`"),
        "package" => format!("Create a release or distribution artifact via `{command}`"),
        _ => format!("Run `{command}`"),
    }
}

fn newest_mtime(path: &Path, manifests: &[String], readme: Option<&str>) -> Option<String> {
    let mut latest: Option<SystemTime> = None;
    let mut consider = |file: &Path| {
        if let Ok(meta) = file.metadata() {
            if let Ok(modified) = meta.modified() {
                latest = Some(match latest {
                    Some(current) if current >= modified => current,
                    _ => modified,
                });
            }
        }
    };
    consider(path);
    for manifest in manifests {
        consider(&path.join(manifest));
    }
    if let Some(readme) = readme {
        consider(&path.join(readme));
    }
    latest.and_then(|time| {
        chrono::DateTime::<Utc>::from(time)
            .to_rfc3339_opts(SecondsFormat::Secs, true)
            .into()
    })
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn detect_js_runtime(path: &Path, facts: &mut Vec<DetectedFact>) {
    if let Some(pkg) = read_json(path.join("package.json")) {
        if let Some(node) = pkg
            .get("engines")
            .and_then(|value| value.get("node"))
            .and_then(Value::as_str)
        {
            push_runtime(facts, "node", node, "package.json#engines.node");
        }
        if let Some(volta) = pkg
            .get("volta")
            .and_then(|value| value.get("node"))
            .and_then(Value::as_str)
        {
            push_runtime(facts, "node", volta, "package.json#volta.node");
        }
        if let Some(manager) = pkg.get("packageManager").and_then(Value::as_str) {
            push_fact(
                facts,
                "packageManager",
                manager,
                0.9,
                "package.json#packageManager",
            );
        }
    }
}

fn detect_python_runtime(path: &Path, pyproject: &str, facts: &mut Vec<DetectedFact>) {
    if let Ok(value) = pyproject.parse::<toml::Value>() {
        if let Some(requires) = value
            .get("project")
            .and_then(|project| project.get("requires-python"))
            .and_then(|item| item.as_str())
        {
            push_runtime(
                facts,
                "python",
                requires,
                "pyproject.toml#project.requires-python",
            );
        }
    }
    if let Some(version) = first_existing_text(path, &[".python-version"]) {
        push_runtime(facts, "python", version.trim(), ".python-version");
    }
}

fn detect_rust_toolchain(path: &Path, facts: &mut Vec<DetectedFact>) {
    if let Some(text) = first_existing_text(path, &["rust-toolchain", "rust-toolchain.toml"]) {
        if let Ok(value) = text.parse::<toml::Value>() {
            if let Some(channel) = value
                .get("toolchain")
                .and_then(|toolchain| toolchain.get("channel"))
                .and_then(|item| item.as_str())
            {
                push_runtime(facts, "rust", channel, "rust-toolchain.toml");
                return;
            }
        }
        let channel = text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .trim();
        if !channel.is_empty() {
            push_runtime(facts, "rust", channel, "rust-toolchain");
        }
    }
}

fn detect_version_files(path: &Path, facts: &mut Vec<DetectedFact>) {
    if let Some(version) = first_existing_text(path, &[".nvmrc", ".node-version"]) {
        push_runtime(facts, "node", version.trim(), ".nvmrc");
    }
    if let Some(text) = first_existing_text(path, &[".tool-versions"]) {
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(tool), Some(version)) = (parts.next(), parts.next()) {
                let ecosystem = match tool {
                    "nodejs" | "node" => "node",
                    "python" => "python",
                    "rust" => "rust",
                    "java" => "java",
                    "golang" | "go" => "go",
                    _ => continue,
                };
                push_runtime(facts, ecosystem, version, ".tool-versions");
            }
        }
    }
}

fn detect_project_icon(path: &Path, facts: &mut Vec<DetectedFact>) {
    let mut candidates = Vec::new();
    if let Some(pkg) = read_json(path.join("package.json")) {
        collect_json_icon(&mut candidates, &pkg, "package.json");
    }
    if let Some(tauri) = read_json(path.join("src-tauri/tauri.conf.json"))
        .or_else(|| read_json(path.join("tauri.conf.json")))
    {
        if let Some(icons) = tauri.pointer("/bundle/icon").and_then(Value::as_array) {
            for icon in icons.iter().filter_map(Value::as_str) {
                candidates.push((icon.to_string(), "tauri.conf.json#bundle.icon".into()));
            }
        }
    }
    if let Some(html) = first_existing_text(path, &["index.html", "public/index.html"]) {
        for href in html_icon_hrefs(&html) {
            candidates.push((href, "index.html".into()));
        }
    }
    const CONVENTIONAL: &[&str] = &[
        "favicon.png",
        "favicon.ico",
        "icon.png",
        "logo.png",
        "public/favicon.png",
        "public/favicon.ico",
        "public/icon.png",
        "public/logo.png",
        "static/favicon.png",
        "static/icon.png",
        "src/assets/logo.png",
        "src/assets/icon.png",
        "assets/logo.png",
        "src-tauri/icons/icon.png",
        "src-tauri/icons/32x32.png",
    ];
    for item in CONVENTIONAL {
        candidates.push(((*item).into(), "convention".into()));
    }
    for (relative, source) in candidates {
        let cleaned = relative.trim_start_matches("./").replace("\\", "/");
        if cleaned.is_empty() || cleaned.contains("..") {
            continue;
        }
        let file = path.join(&cleaned);
        if !file.is_file() {
            continue;
        }
        let lower = cleaned.to_ascii_lowercase();
        if !(lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".webp")
            || lower.ends_with(".ico"))
        {
            continue;
        }
        push_fact(facts, "asset", &cleaned, 0.8, &source);
        break;
    }
}

fn collect_json_icon(out: &mut Vec<(String, String)>, value: &Value, source: &str) {
    for key in ["icon", "favicon", "logo"] {
        if let Some(path) = value.get(key).and_then(Value::as_str) {
            out.push((path.to_string(), format!("{source}#{key}")));
        }
    }
}

fn html_icon_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut rest = html;
    let mut lower_rest = lower.as_str();
    while let Some(start) = lower_rest.find("<link") {
        let chunk = &rest[start..];
        let lower_chunk = &lower_rest[start..];
        let end = lower_chunk.find('>').unwrap_or(chunk.len());
        let tag = &chunk[..end];
        let tag_lower = tag.to_ascii_lowercase();
        if tag_lower.contains("rel=\"icon\"")
            || tag_lower.contains("rel='icon'")
            || tag_lower.contains("rel=\"shortcut icon\"")
            || tag_lower.contains("rel=\"apple-touch-icon\"")
        {
            if let Some(href) = attr_value(tag, "href") {
                hrefs.push(href);
            }
        }
        rest = &chunk[end.min(chunk.len())..];
        lower_rest = &lower_chunk[end.min(lower_chunk.len())..];
    }
    hrefs
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=");
    let lower = tag.to_ascii_lowercase();
    let index = lower.find(&pattern)?;
    let after = tag.get(index + pattern.len()..)?;
    let quote = after.chars().next()?;
    if quote == '"' || quote == '\'' {
        after[1..]
            .split(quote)
            .next()
            .map(|value| value.to_string())
    } else {
        after
            .split_whitespace()
            .next()
            .map(|value| value.to_string())
    }
}

fn first_existing_text(path: &Path, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| read_text(&path.join(name)))
}

fn read_text(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_DETECT_TEXT_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_DETECT_TEXT_BYTES || bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn go_directive(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("go ")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn gradle_java_version(source: &str) -> Option<String> {
    for pattern in [
        "sourceCompatibility = ",
        "targetCompatibility = ",
        "languageVersion.set(JavaLanguageVersion.of(",
        "jvmTarget.set(\"",
    ] {
        if let Some(rest) = source.split(pattern).nth(1) {
            let value = rest
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == '_')
                .collect::<String>();
            if !value.is_empty() {
                return Some(value.replace('_', "."));
            }
        }
    }
    None
}

fn yaml_scalar(source: &str, key: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix(&format!("{key}:"))
            .map(|value| {
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string()
            })
            .filter(|value| !value.is_empty())
    })
}

fn push_runtime(facts: &mut Vec<DetectedFact>, ecosystem: &str, constraint: &str, source: &str) {
    let value = format!("{ecosystem}@{constraint}");
    push_fact(facts, "runtime", &value, 0.9, source);
}
