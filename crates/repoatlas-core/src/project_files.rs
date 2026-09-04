use crate::{environment, paths, Error, Result};
use cap_fs_ext::DirExt;
use cap_std::{ambient_authority, fs::Dir};
use ignore::{DirEntry, WalkBuilder, WalkState};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering},
        Arc, Mutex,
    },
};

const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_MARKDOWN_BYTES: usize = 1024 * 1024;
const MAX_TEXT_LINES: usize = 20_000;
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_IMAGE_PIXELS: usize = 40_000_000;
const MAX_DIRECTORY_ENTRIES: usize = 20_000;
const MAX_INDEX_ENTRIES: usize = 1_000_000;
const MAX_INDEX_MEMORY_BYTES: usize = 128 * 1024 * 1024;

const VCS_DIRS: &[&str] = &[".git", ".svn", ".hg"];
const GENERATED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    "coverage",
    "__pycache__",
    ".venv",
    "venv",
    "vendor",
    "bin",
    "obj",
    ".gradle",
    ".dart_tool",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectFileEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirectoryEntry {
    pub name: String,
    pub path: String,
    pub kind: ProjectFileEntryKind,
    pub generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirectoryListing {
    pub path: String,
    pub entries: Vec<ProjectDirectoryEntry>,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectFilePreviewKind {
    Markdown,
    Code,
    Text,
    Image,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFilePreview {
    pub path: String,
    pub kind: ProjectFilePreviewKind,
    pub size: u64,
    pub mime: Option<String>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub truncated: bool,
    pub width: Option<usize>,
    pub height: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathIndexEntry {
    pub name: String,
    pub path: String,
    pub kind: ProjectFileEntryKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathIndex {
    pub entries: Vec<ProjectPathIndexEntry>,
    pub scanned_count: usize,
    pub limited: bool,
    pub canceled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathSearchResult {
    pub name: String,
    pub path: String,
    pub kind: ProjectFileEntryKind,
    pub score: u32,
}

#[derive(Clone, Copy)]
struct PathSearchCandidate {
    entry_index: usize,
    score: u32,
    depth: usize,
    path_len: usize,
}

pub fn list_directory(
    project_root: &Path,
    relative_path: &str,
    show_generated: bool,
) -> Result<ProjectDirectoryListing> {
    let relative = validate_relative(relative_path, true)?;
    let directory = open_project_directory(project_root, &relative)?;
    let mut entries = Vec::new();
    let mut skipped_count = 0;

    for next in directory.entries()? {
        let entry = match next {
            Ok(entry) => entry,
            Err(_) => {
                skipped_count += 1;
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        let file_type = match entry.file_type() {
            Ok(value) => value,
            Err(_) => {
                skipped_count += 1;
                continue;
            }
        };
        let kind = if file_type.is_symlink() {
            ProjectFileEntryKind::Symlink
        } else if file_type.is_dir() {
            ProjectFileEntryKind::Directory
        } else if file_type.is_file() {
            ProjectFileEntryKind::File
        } else {
            ProjectFileEntryKind::Other
        };
        if kind == ProjectFileEntryKind::Directory && is_vcs_dir(&lower) {
            skipped_count += 1;
            continue;
        }
        let generated = kind == ProjectFileEntryKind::Directory && is_generated_dir(&lower);
        if generated && !show_generated {
            skipped_count += 1;
            continue;
        }
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            skipped_count += 1;
            continue;
        }
        entries.push(ProjectDirectoryEntry {
            path: join_relative(&relative, &name),
            name,
            kind,
            generated,
        });
    }

    entries.sort_unstable_by(compare_directory_entries);
    Ok(ProjectDirectoryListing {
        path: normalize_relative(&relative),
        entries,
        skipped_count,
    })
}

pub fn read_preview(project_root: &Path, relative_path: &str) -> Result<ProjectFilePreview> {
    let relative = validate_relative(relative_path, false)?;
    let mut file = environment::open_regular_project_file(project_root, &relative)?;
    let size = file.metadata()?.len();
    let mut bytes = Vec::with_capacity((size.min((MAX_TEXT_BYTES + 1) as u64)) as usize);
    file.by_ref()
        .take((MAX_TEXT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let detected = infer::get(&bytes);
    let mime = detected.map(|kind| kind.mime_type().to_string());
    let normalized_path = normalize_relative(&relative);

    if let Some(image_mime) = mime
        .as_deref()
        .filter(|value| is_supported_image_mime(value))
    {
        let dimensions = imagesize::blob_size(&bytes).ok();
        let (width, height) = dimensions
            .map(|value| (Some(value.width), Some(value.height)))
            .unwrap_or((None, None));
        let too_large = size > MAX_IMAGE_BYTES
            || width.zip(height).is_some_and(|(w, h)| {
                w.checked_mul(h)
                    .is_none_or(|pixels| pixels > MAX_IMAGE_PIXELS)
            });
        return Ok(ProjectFilePreview {
            path: normalized_path,
            kind: if too_large {
                ProjectFilePreviewKind::Unsupported
            } else {
                ProjectFilePreviewKind::Image
            },
            size,
            mime: Some(image_mime.to_string()),
            language: None,
            content: None,
            truncated: false,
            width,
            height,
            message: too_large.then(|| "image exceeds the safe preview limit".to_string()),
        });
    }

    let Some(mut content) = decode_text_sample(&bytes) else {
        return Ok(ProjectFilePreview {
            path: normalized_path,
            kind: ProjectFilePreviewKind::Unsupported,
            size,
            mime,
            language: None,
            content: None,
            truncated: false,
            width: None,
            height: None,
            message: Some("binary files cannot be previewed".into()),
        });
    };

    let mut truncated = bytes.len() > MAX_TEXT_BYTES;
    if let Some(byte_index) = nth_line_byte_index(&content, MAX_TEXT_LINES) {
        content.truncate(byte_index);
        truncated = true;
    }
    let language = language_for_path(&relative);
    let markdown = is_markdown_path(&relative) && size <= MAX_MARKDOWN_BYTES as u64;
    let kind = if markdown {
        ProjectFilePreviewKind::Markdown
    } else if language.is_some() || is_markdown_path(&relative) {
        ProjectFilePreviewKind::Code
    } else {
        ProjectFilePreviewKind::Text
    };
    let text_mime = if is_markdown_path(&relative) {
        "text/markdown".to_string()
    } else if language.as_deref() == Some("html") {
        "text/html".to_string()
    } else {
        mime.filter(|value| value.starts_with("text/"))
            .unwrap_or_else(|| "text/plain".to_string())
    };
    Ok(ProjectFilePreview {
        path: normalized_path,
        kind,
        size,
        mime: Some(text_mime),
        language,
        content: Some(content),
        truncated,
        width: None,
        height: None,
        message: None,
    })
}

pub fn read_image(project_root: &Path, relative_path: &str) -> Result<Vec<u8>> {
    let relative = validate_relative(relative_path, false)?;
    let mut file = environment::open_regular_project_file(project_root, &relative)?;
    let size = file.metadata()?.len();
    if size > MAX_IMAGE_BYTES {
        return Err(Error::msg("image exceeds the safe preview limit"));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.read_to_end(&mut bytes)?;
    let mime = infer::get(&bytes)
        .map(|kind| kind.mime_type())
        .filter(|value| is_supported_image_mime(value))
        .ok_or_else(|| Error::msg("unsupported image format"))?;
    let _ = mime;
    let dimensions = imagesize::blob_size(&bytes)
        .map_err(|_| Error::msg("image dimensions could not be read"))?;
    if dimensions
        .width
        .checked_mul(dimensions.height)
        .is_none_or(|pixels| pixels > MAX_IMAGE_PIXELS)
    {
        return Err(Error::msg("image exceeds the safe preview limit"));
    }
    Ok(bytes)
}

pub fn build_path_index(
    project_root: &Path,
    include_generated: bool,
    cancel: Arc<AtomicBool>,
    scanned: Arc<AtomicUsize>,
    indexed: Arc<AtomicUsize>,
) -> Result<ProjectPathIndex> {
    let root = paths::canonicalize(project_root)?;
    let output = Arc::new(Mutex::new(Vec::new()));
    let reserved = Arc::new(AtomicUsize::new(0));
    let memory_bytes = Arc::new(AtomicUsize::new(0));
    let limited = Arc::new(AtomicBool::new(false));
    let threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2)
        .clamp(1, 8);

    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .threads(threads);
    let walker = builder.build_parallel();
    walker.run(|| {
        let root = root.clone();
        let output = Arc::clone(&output);
        let cancel = Arc::clone(&cancel);
        let scanned = Arc::clone(&scanned);
        let indexed = Arc::clone(&indexed);
        let reserved = Arc::clone(&reserved);
        let memory_bytes = Arc::clone(&memory_bytes);
        let limited = Arc::clone(&limited);
        let mut batch = IndexBatch::new(output);
        Box::new(move |result| {
            if cancel.load(AtomicOrdering::Relaxed) || limited.load(AtomicOrdering::Relaxed) {
                return WalkState::Quit;
            }
            let Ok(entry) = result else {
                return WalkState::Continue;
            };
            if entry.depth() == 0 {
                return WalkState::Continue;
            }
            scanned.fetch_add(1, AtomicOrdering::Relaxed);
            let Some(relative) = entry.path().strip_prefix(&root).ok() else {
                return WalkState::Continue;
            };
            let is_directory = entry.file_type().is_some_and(|kind| kind.is_dir());
            let lower_name = entry
                .path()
                .file_name()
                .map(|value| value.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if is_directory
                && (is_vcs_dir(&lower_name)
                    || (is_generated_dir(&lower_name) && !include_generated))
            {
                return WalkState::Skip;
            }
            let path = normalize_relative(relative);
            let name = entry
                .path()
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            let entry_bytes = std::mem::size_of::<ProjectPathIndexEntry>()
                .saturating_add(path.len())
                .saturating_add(name.len());
            let next_count = reserved.fetch_add(1, AtomicOrdering::Relaxed) + 1;
            let next_bytes =
                memory_bytes.fetch_add(entry_bytes, AtomicOrdering::Relaxed) + entry_bytes;
            if next_count > MAX_INDEX_ENTRIES || next_bytes > MAX_INDEX_MEMORY_BYTES {
                limited.store(true, AtomicOrdering::Relaxed);
                return WalkState::Quit;
            }
            batch.push(ProjectPathIndexEntry {
                name,
                path,
                kind: entry_kind(&entry),
            });
            indexed.fetch_add(1, AtomicOrdering::Relaxed);
            WalkState::Continue
        })
    });

    let mut entries = Arc::try_unwrap(output)
        .map_err(|_| Error::msg("path index is still in use"))?
        .into_inner()
        .map_err(|_| Error::msg("path index lock was poisoned"))?;
    entries.sort_unstable_by(|left, right| natural_cmp(&left.path, &right.path));
    Ok(ProjectPathIndex {
        entries,
        scanned_count: scanned.load(AtomicOrdering::Relaxed),
        limited: limited.load(AtomicOrdering::Relaxed),
        canceled: cancel.load(AtomicOrdering::Relaxed),
    })
}

pub fn search_path_index(
    index: &ProjectPathIndex,
    query: &str,
    limit: usize,
) -> Vec<ProjectPathSearchResult> {
    let cancel = AtomicBool::new(false);
    search_path_index_cancellable(index, query, limit, &cancel)
}

pub fn search_path_index_cancellable(
    index: &ProjectPathIndex,
    query: &str,
    limit: usize,
    cancel: &AtomicBool,
) -> Vec<ProjectPathSearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let lowercase_query = (!query.is_ascii()).then(|| query.to_lowercase());
    let result_limit = limit.min(200);
    if result_limit == 0 {
        return Vec::new();
    }

    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(8);
    let mut candidates = if index.entries.len() >= 25_000 && worker_count > 1 {
        std::thread::scope(|scope| {
            let chunk_size = index.entries.len().div_ceil(worker_count);
            let mut handles = Vec::with_capacity(worker_count);
            for (chunk_index, entries) in index.entries.chunks(chunk_size).enumerate() {
                let entry_offset = chunk_index * chunk_size;
                let lowercase_query = lowercase_query.as_deref();
                handles.push(scope.spawn(move || {
                    collect_search_candidates(entries, entry_offset, query, lowercase_query, cancel)
                }));
            }
            let mut candidates = Vec::new();
            for handle in handles {
                candidates.extend(handle.join().expect("path search worker panicked"));
            }
            candidates
        })
    } else {
        collect_search_candidates(&index.entries, 0, query, lowercase_query.as_deref(), cancel)
    };

    if cancel.load(AtomicOrdering::Relaxed) {
        return Vec::new();
    }

    let compare = |left: &PathSearchCandidate, right: &PathSearchCandidate| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.depth.cmp(&right.depth))
            .then_with(|| left.path_len.cmp(&right.path_len))
            .then_with(|| {
                natural_cmp(
                    &index.entries[left.entry_index].path,
                    &index.entries[right.entry_index].path,
                )
            })
    };
    if candidates.len() > result_limit {
        candidates.select_nth_unstable_by(result_limit, compare);
        candidates.truncate(result_limit);
    }
    candidates.sort_unstable_by(compare);
    candidates
        .into_iter()
        .map(|candidate| {
            let entry = &index.entries[candidate.entry_index];
            ProjectPathSearchResult {
                name: entry.name.clone(),
                path: entry.path.clone(),
                kind: entry.kind,
                score: candidate.score,
            }
        })
        .collect()
}

fn collect_search_candidates(
    entries: &[ProjectPathIndexEntry],
    entry_offset: usize,
    query: &str,
    lowercase_query: Option<&str>,
    cancel: &AtomicBool,
) -> Vec<PathSearchCandidate> {
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let mut needle_buf = Vec::new();
    let needle = Utf32Str::new(query, &mut needle_buf);
    let mut haystack_buf = Vec::new();
    let mut candidates = Vec::new();

    for (chunk_entry_index, entry) in entries.iter().enumerate() {
        if chunk_entry_index % 128 == 0 && cancel.load(AtomicOrdering::Relaxed) {
            break;
        }
        let entry_index = entry_offset + chunk_entry_index;
        let (exact_name, prefix_name) = if query.is_ascii() && entry.name.is_ascii() {
            (
                entry.name.eq_ignore_ascii_case(query),
                entry
                    .name
                    .get(..query.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(query)),
            )
        } else {
            let lower_name = entry.name.to_lowercase();
            let lower_query = lowercase_query.unwrap_or(query);
            (
                lower_name == lower_query,
                lower_name.starts_with(lower_query),
            )
        };
        if exact_name {
            candidates.push(PathSearchCandidate {
                entry_index,
                score: 1_000_000,
                depth: entry
                    .path
                    .as_bytes()
                    .iter()
                    .filter(|&&byte| byte == b'/')
                    .count(),
                path_len: entry.path.len(),
            });
            continue;
        }
        if prefix_name {
            candidates.push(PathSearchCandidate {
                entry_index,
                score: 900_000u32.saturating_sub(entry.name.len() as u32),
                depth: entry
                    .path
                    .as_bytes()
                    .iter()
                    .filter(|&&byte| byte == b'/')
                    .count(),
                path_len: entry.path.len(),
            });
            continue;
        }
        let name_haystack = Utf32Str::new(&entry.name, &mut haystack_buf);
        let name_fuzzy = matcher.fuzzy_match(name_haystack, needle);
        let score = if let Some(fuzzy) = name_fuzzy {
            500_000u32.saturating_add(fuzzy as u32)
        } else {
            let path_haystack = Utf32Str::new(&entry.path, &mut haystack_buf);
            let Some(path_fuzzy) = matcher.fuzzy_match(path_haystack, needle) else {
                continue;
            };
            100_000u32.saturating_add(path_fuzzy as u32)
        };
        candidates.push(PathSearchCandidate {
            entry_index,
            score,
            depth: entry
                .path
                .as_bytes()
                .iter()
                .filter(|&&byte| byte == b'/')
                .count(),
            path_len: entry.path.len(),
        });
    }
    candidates
}

fn open_project_directory(project_root: &Path, relative: &Path) -> Result<Dir> {
    let root = paths::canonicalize(project_root)?;
    let mut directory = Dir::open_ambient_dir(root, ambient_authority())?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(Error::msg("directory path is outside the project"));
        };
        directory = directory
            .open_dir_nofollow(name)
            .map_err(|_| Error::msg("directory path is outside the project"))?;
    }
    Ok(directory)
}

pub fn validate_existing_path(project_root: &Path, relative_path: &str) -> Result<PathBuf> {
    let relative = validate_relative(relative_path, false)?;
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let file_name = relative
        .file_name()
        .ok_or_else(|| Error::msg("file path is outside the project"))?;
    let directory = open_project_directory(project_root, parent)?;
    let metadata = directory.symlink_metadata(file_name)?;
    if metadata.file_type().is_symlink() {
        return Err(Error::msg("symbolic links cannot be opened"));
    }
    Ok(project_root.join(relative))
}

fn validate_relative(value: &str, allow_empty: bool) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute()
        || (!allow_empty && path.as_os_str().is_empty())
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(Error::msg("file path is outside the project"));
    }
    Ok(path.to_path_buf())
}

fn normalize_relative(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn join_relative(parent: &Path, name: &str) -> String {
    let parent = normalize_relative(parent);
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

fn is_vcs_dir(lower_name: &str) -> bool {
    VCS_DIRS.contains(&lower_name)
}

fn is_generated_dir(lower_name: &str) -> bool {
    GENERATED_DIRS.contains(&lower_name)
}

fn entry_kind(entry: &DirEntry) -> ProjectFileEntryKind {
    match entry.file_type() {
        Some(kind) if kind.is_symlink() => ProjectFileEntryKind::Symlink,
        Some(kind) if kind.is_dir() => ProjectFileEntryKind::Directory,
        Some(kind) if kind.is_file() => ProjectFileEntryKind::File,
        _ => ProjectFileEntryKind::Other,
    }
}

fn compare_directory_entries(
    left: &ProjectDirectoryEntry,
    right: &ProjectDirectoryEntry,
) -> Ordering {
    let left_rank = match left.kind {
        ProjectFileEntryKind::Directory => 0,
        ProjectFileEntryKind::File => 1,
        ProjectFileEntryKind::Symlink => 2,
        ProjectFileEntryKind::Other => 3,
    };
    let right_rank = match right.kind {
        ProjectFileEntryKind::Directory => 0,
        ProjectFileEntryKind::File => 1,
        ProjectFileEntryKind::Symlink => 2,
        ProjectFileEntryKind::Other => 3,
    };
    left_rank
        .cmp(&right_rank)
        .then_with(|| natural_cmp(&left.name, &right.name))
        .then_with(|| left.path.cmp(&right.path))
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left_chars = left.chars().flat_map(char::to_lowercase).peekable();
    let mut right_chars = right.chars().flat_map(char::to_lowercase).peekable();
    loop {
        match (left_chars.peek().copied(), right_chars.peek().copied()) {
            (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                let mut left_number = String::new();
                let mut right_number = String::new();
                while left_chars.peek().is_some_and(char::is_ascii_digit) {
                    left_number.push(left_chars.next().unwrap());
                }
                while right_chars.peek().is_some_and(char::is_ascii_digit) {
                    right_number.push(right_chars.next().unwrap());
                }
                let left_trimmed = left_number.trim_start_matches('0');
                let right_trimmed = right_number.trim_start_matches('0');
                let order = left_trimmed
                    .len()
                    .cmp(&right_trimmed.len())
                    .then_with(|| left_trimmed.cmp(right_trimmed))
                    .then_with(|| left_number.len().cmp(&right_number.len()));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(a), Some(b)) => {
                left_chars.next();
                right_chars.next();
                let order = a.cmp(&b);
                if order != Ordering::Equal {
                    return order;
                }
            }
            (None, None) => return left.cmp(right),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown" | "mdown" | "mkd")
    )
}

fn language_for_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    let language = match file_name.as_str() {
        "dockerfile" => "docker",
        "makefile" | "gnumakefile" => "make",
        "cargo.toml" | "pyproject.toml" => "toml",
        _ => match path
            .extension()?
            .to_string_lossy()
            .to_ascii_lowercase()
            .as_str()
        {
            "rs" => "rust",
            "ts" => "typescript",
            "tsx" => "tsx",
            "js" | "mjs" | "cjs" => "javascript",
            "jsx" => "jsx",
            "json" | "jsonc" => "json",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            "xml" => "xml",
            "html" | "htm" => "html",
            "css" => "css",
            "scss" => "scss",
            "less" => "less",
            "py" => "python",
            "go" => "go",
            "java" => "java",
            "kt" | "kts" => "kotlin",
            "swift" => "swift",
            "c" | "h" => "c",
            "cc" | "cpp" | "cxx" | "hpp" => "cpp",
            "cs" => "csharp",
            "sh" | "bash" | "zsh" => "bash",
            "ps1" | "psm1" => "powershell",
            "sql" => "sql",
            "rb" => "ruby",
            "php" => "php",
            "dart" => "dart",
            "vue" => "vue",
            "svelte" => "svelte",
            "md" | "markdown" | "mdown" | "mkd" => "markdown",
            "diff" | "patch" => "diff",
            _ => return None,
        },
    };
    Some(language.to_string())
}

fn nth_line_byte_index(content: &str, max_lines: usize) -> Option<usize> {
    let mut lines = 0;
    for (index, ch) in content.char_indices() {
        if ch == '\n' {
            lines += 1;
            if lines >= max_lines {
                return Some(index + 1);
            }
        }
    }
    None
}

fn decode_text_sample(bytes: &[u8]) -> Option<String> {
    let decoded = if let Some(body) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        decode_utf8_sample(body)?
    } else if let Some(body) = bytes.strip_prefix(&[0xff, 0xfe]) {
        let units = body
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).ok()?
    } else if let Some(body) = bytes.strip_prefix(&[0xfe, 0xff]) {
        let units = body
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).ok()?
    } else {
        decode_utf8_sample(bytes)?
    };

    let mut characters = 0usize;
    let mut unsafe_controls = 0usize;
    for character in decoded.chars() {
        characters += 1;
        if character == '\0' {
            return None;
        }
        if character.is_control() && !matches!(character, '\n' | '\r' | '\t' | '\u{000c}') {
            unsafe_controls += 1;
        }
    }
    if unsafe_controls > 2 && unsafe_controls.saturating_mul(100) > characters.max(1) {
        None
    } else {
        Some(decoded)
    }
}

fn decode_utf8_sample(bytes: &[u8]) -> Option<String> {
    match std::str::from_utf8(bytes) {
        Ok(value) => Some(value.to_owned()),
        Err(error)
            if error.error_len().is_none()
                && bytes.len().saturating_sub(error.valid_up_to()) <= 3 =>
        {
            Some(
                std::str::from_utf8(&bytes[..error.valid_up_to()])
                    .ok()?
                    .to_owned(),
            )
        }
        Err(_) => None,
    }
}

fn is_supported_image_mime(mime: &str) -> bool {
    matches!(
        mime,
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/bmp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
    )
}

struct IndexBatch {
    output: Arc<Mutex<Vec<ProjectPathIndexEntry>>>,
    entries: Vec<ProjectPathIndexEntry>,
}

impl IndexBatch {
    fn new(output: Arc<Mutex<Vec<ProjectPathIndexEntry>>>) -> Self {
        Self {
            output,
            entries: Vec::with_capacity(256),
        }
    }

    fn push(&mut self, entry: ProjectPathIndexEntry) {
        self.entries.push(entry);
        if self.entries.len() >= 256 {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        if let Ok(mut output) = self.output.lock() {
            output.append(&mut self.entries);
        }
    }
}

impl Drop for IndexBatch {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn lists_one_level_with_filters_and_natural_sorting() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("folder10")).unwrap();
        fs::create_dir(root.path().join("folder2")).unwrap();
        fs::create_dir(root.path().join("node_modules")).unwrap();
        fs::create_dir(root.path().join(".git")).unwrap();
        fs::write(root.path().join("readme.md"), "hello").unwrap();

        let listing = list_directory(root.path(), "", false).unwrap();
        let names: Vec<_> = listing
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, ["folder2", "folder10", "readme.md"]);
        assert_eq!(listing.skipped_count, 2);

        let listing = list_directory(root.path(), "", true).unwrap();
        assert!(listing
            .entries
            .iter()
            .any(|entry| entry.name == "node_modules" && entry.generated));
        assert!(!listing.entries.iter().any(|entry| entry.name == ".git"));
    }

    #[test]
    fn preview_classifies_markdown_code_and_binary() {
        let root = tempdir().unwrap();
        fs::write(
            root.path().join("README.md"),
            "<p align=\"center\"><img src=\"logo.png\" /></p>\n# Hello",
        )
        .unwrap();
        fs::write(
            root.path().join("index.html"),
            "<!doctype html><html><body>Hello</body></html>",
        )
        .unwrap();
        fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.path().join("data.bin"), [0, 1, 2]).unwrap();

        let markdown = read_preview(root.path(), "README.md").unwrap();
        assert_eq!(markdown.kind, ProjectFilePreviewKind::Markdown);
        assert_eq!(markdown.mime.as_deref(), Some("text/markdown"));
        let html = read_preview(root.path(), "index.html").unwrap();
        assert_eq!(html.kind, ProjectFilePreviewKind::Code);
        assert_eq!(html.mime.as_deref(), Some("text/html"));
        assert_eq!(
            read_preview(root.path(), "main.rs").unwrap().kind,
            ProjectFilePreviewKind::Code
        );
        assert_eq!(
            read_preview(root.path(), "data.bin").unwrap().kind,
            ProjectFilePreviewKind::Unsupported
        );
    }

    #[test]
    fn preview_decodes_utf16_text_but_rejects_control_heavy_binary() {
        let root = tempdir().unwrap();
        let mut utf16 = vec![0xff, 0xfe];
        for unit in "# UTF-16 markdown".encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(root.path().join("utf16.md"), utf16).unwrap();
        fs::write(
            root.path().join("renamed.md"),
            b"PK\x03\x04\x01\x02\x03\x04",
        )
        .unwrap();

        let preview = read_preview(root.path(), "utf16.md").unwrap();
        assert_eq!(preview.kind, ProjectFilePreviewKind::Markdown);
        assert_eq!(preview.content.as_deref(), Some("# UTF-16 markdown"));
        assert_eq!(
            read_preview(root.path(), "renamed.md").unwrap().kind,
            ProjectFilePreviewKind::Unsupported
        );
    }

    #[test]
    fn index_ignores_vcs_and_optionally_generated_directories() {
        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::create_dir_all(root.path().join("target/debug")).unwrap();
        fs::create_dir_all(root.path().join(".git/objects")).unwrap();
        fs::write(root.path().join("src/main.rs"), "").unwrap();
        fs::write(root.path().join("target/debug/app.exe"), "").unwrap();
        fs::write(root.path().join(".git/HEAD"), "").unwrap();

        let index = build_path_index(
            root.path(),
            false,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
        )
        .unwrap();
        assert!(index
            .entries
            .iter()
            .any(|entry| entry.path == "src/main.rs"));
        assert!(!index
            .entries
            .iter()
            .any(|entry| entry.path.starts_with("target/")));
        assert!(!index
            .entries
            .iter()
            .any(|entry| entry.path.starts_with(".git/")));

        let index = build_path_index(
            root.path(),
            true,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
        )
        .unwrap();
        assert!(index
            .entries
            .iter()
            .any(|entry| entry.path == "target/debug/app.exe"));
        assert!(!index
            .entries
            .iter()
            .any(|entry| entry.path.starts_with(".git/")));
    }

    #[test]
    fn fuzzy_search_prefers_exact_file_names() {
        let index = ProjectPathIndex {
            entries: vec![
                ProjectPathIndexEntry {
                    name: "main.rs".into(),
                    path: "examples/main.rs".into(),
                    kind: ProjectFileEntryKind::File,
                },
                ProjectPathIndexEntry {
                    name: "domain.rs".into(),
                    path: "src/main/domain.rs".into(),
                    kind: ProjectFileEntryKind::File,
                },
            ],
            scanned_count: 2,
            limited: false,
            canceled: false,
        };
        let results = search_path_index(&index, "main.rs", 20);
        assert_eq!(
            results.first().map(|item| item.path.as_str()),
            Some("examples/main.rs")
        );
    }

    #[test]
    fn rejects_parent_paths() {
        let root = tempdir().unwrap();
        assert!(list_directory(root.path(), "../", false).is_err());
        assert!(read_preview(root.path(), "../secret.txt").is_err());
    }

    #[test]
    fn gitignore_does_not_hide_real_files() {
        let root = tempdir().unwrap();
        fs::write(root.path().join(".gitignore"), "hidden.txt\n").unwrap();
        fs::write(root.path().join("hidden.txt"), "still visible").unwrap();

        let listing = list_directory(root.path(), "", false).unwrap();
        assert!(listing
            .entries
            .iter()
            .any(|entry| entry.name == "hidden.txt"));
        let index = build_path_index(
            root.path(),
            false,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
        )
        .unwrap();
        assert!(index.entries.iter().any(|entry| entry.path == "hidden.txt"));
    }

    #[test]
    fn text_preview_stops_at_the_line_limit() {
        let root = tempdir().unwrap();
        fs::write(
            root.path().join("large.txt"),
            "line\n".repeat(MAX_TEXT_LINES + 5),
        )
        .unwrap();
        let preview = read_preview(root.path(), "large.txt").unwrap();
        assert!(preview.truncated);
        assert_eq!(preview.content.unwrap().lines().count(), MAX_TEXT_LINES);
    }

    #[test]
    fn oversized_image_dimensions_are_rejected_before_decode() {
        let root = tempdir().unwrap();
        let mut png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R',
        ];
        png.extend_from_slice(&10_000u32.to_be_bytes());
        png.extend_from_slice(&5_000u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        fs::write(root.path().join("large.png"), png).unwrap();

        let preview = read_preview(root.path(), "large.png").unwrap();
        assert_eq!(preview.kind, ProjectFilePreviewKind::Unsupported);
        assert_eq!((preview.width, preview.height), (Some(10_000), Some(5_000)));
        assert!(read_image(root.path(), "large.png").is_err());
    }

    #[test]
    fn fuzzy_search_handles_chinese_paths_case_insensitively() {
        let index = ProjectPathIndex {
            entries: vec![
                ProjectPathIndexEntry {
                    name: "用户设置.ts".into(),
                    path: "src/页面/用户设置.ts".into(),
                    kind: ProjectFileEntryKind::File,
                },
                ProjectPathIndexEntry {
                    name: "SettingsPanel.tsx".into(),
                    path: "src/SettingsPanel.tsx".into(),
                    kind: ProjectFileEntryKind::File,
                },
            ],
            scanned_count: 2,
            limited: false,
            canceled: false,
        };
        assert_eq!(search_path_index(&index, "用户", 20)[0].name, "用户设置.ts");
        assert_eq!(
            search_path_index(&index, "settings", 20)[0].name,
            "SettingsPanel.tsx"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_directories_are_visible_but_never_followed() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();

        let listing = list_directory(root.path(), "", true).unwrap();
        assert!(listing
            .entries
            .iter()
            .any(|entry| entry.name == "linked" && entry.kind == ProjectFileEntryKind::Symlink));
        assert!(list_directory(root.path(), "linked", true).is_err());
        assert!(read_preview(root.path(), "linked/secret.txt").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn symbolic_link_directories_are_visible_but_never_followed() {
        use std::os::windows::fs::symlink_dir;
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        if symlink_dir(outside.path(), root.path().join("linked")).is_err() {
            return;
        }

        let listing = list_directory(root.path(), "", true).unwrap();
        assert!(listing
            .entries
            .iter()
            .any(|entry| entry.name == "linked" && entry.kind == ProjectFileEntryKind::Symlink));
        assert!(list_directory(root.path(), "linked", true).is_err());
        assert!(read_preview(root.path(), "linked/secret.txt").is_err());
    }

    #[test]
    #[ignore = "manual release-mode performance acceptance"]
    fn lists_ten_thousand_direct_children_for_performance_acceptance() {
        let root = tempdir().unwrap();
        for index in 0..10_000 {
            fs::write(root.path().join(format!("file-{index:05}.txt")), "").unwrap();
        }
        let started = std::time::Instant::now();
        let listing = list_directory(root.path(), "", false).unwrap();
        let elapsed = started.elapsed();
        eprintln!("listed and sorted 10,000 direct children in {elapsed:?}");
        assert_eq!(listing.entries.len(), 10_000);
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "{elapsed:?}"
        );
    }

    #[test]
    #[ignore = "manual release-mode performance acceptance"]
    fn searches_two_hundred_fifty_thousand_paths_for_performance_acceptance() {
        let entries = (0..250_000)
            .map(|index| ProjectPathIndexEntry {
                name: format!("component-{index:06}.tsx"),
                path: format!("packages/package-{index:06}/src/component-{index:06}.tsx"),
                kind: ProjectFileEntryKind::File,
            })
            .collect();
        let index = ProjectPathIndex {
            entries,
            scanned_count: 250_000,
            limited: false,
            canceled: false,
        };
        let mut timings = Vec::new();
        for _ in 0..20 {
            let started = std::time::Instant::now();
            let results = search_path_index(&index, "component-249", 200);
            timings.push(started.elapsed());
            assert!(!results.is_empty());
        }
        timings.sort_unstable();
        eprintln!("250,000-path query P95: {:?}", timings[18]);
        assert!(
            timings[18] < std::time::Duration::from_millis(250),
            "{:?}",
            timings[18]
        );
    }

    #[test]
    fn canceled_path_search_stops_without_results() {
        let index = ProjectPathIndex {
            entries: (0..10_000)
                .map(|index| ProjectPathIndexEntry {
                    name: format!("component-{index}.tsx"),
                    path: format!("src/component-{index}.tsx"),
                    kind: ProjectFileEntryKind::File,
                })
                .collect(),
            scanned_count: 10_000,
            limited: false,
            canceled: false,
        };
        let cancel = AtomicBool::new(true);
        assert!(search_path_index_cancellable(&index, "component", 200, &cancel).is_empty());
    }
}
