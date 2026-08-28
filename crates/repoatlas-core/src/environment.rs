use crate::error::{Error, Result};
use crate::models::{
    DetectedFact, EnvironmentInspection, ProjectDetail, ProjectFile, ProjectIcon, ReadmeDocument,
    RuntimeRequirement, RuntimeStatus,
};
use crate::paths;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_ICON_BYTES: u64 = 512 * 1024;
const MAX_ICON_DIMENSION: u32 = 2048;
const MAX_ICON_PIXELS: u64 = 4_194_304;
const VERSION_TIMEOUT: Duration = Duration::from_secs(2);
const VERSION_CACHE_TTL: Duration = Duration::from_secs(60);
const MAX_VERSION_PROBES: usize = 4;
const MAX_VERSION_OUTPUT_BYTES: u64 = 64 * 1024;

#[derive(Clone)]
struct CachedVersion {
    observed_at: Instant,
    version: Option<String>,
}

fn version_cache() -> &'static Mutex<HashMap<String, CachedVersion>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedVersion>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn project_files_from_facts(facts: &[DetectedFact]) -> Vec<ProjectFile> {
    let mut files = Vec::new();
    for fact in facts {
        let kind = match fact.kind.as_str() {
            "manifest" => "manifest",
            "lockfile" => "lockfile",
            "packageManager" => "packageManager",
            "runtime" => "runtime",
            "asset" => "asset",
            _ => continue,
        };
        let path = if kind == "packageManager" {
            // packageManager facts point at a JSON/TOML key (for example
            // `package.json#packageManager`). The drawer opens the file, so
            // keep only the safe relative file portion just like runtime
            // evidence below.
            runtime_source_path(&fact.source).unwrap_or_else(|| fact.source.clone())
        } else if kind == "runtime" {
            runtime_source_path(&fact.source).unwrap_or_else(|| fact.source.clone())
        } else {
            fact.value.clone()
        };
        if path.trim().is_empty() || path.contains('#') {
            continue;
        }
        files.push(ProjectFile {
            kind: kind.into(),
            path,
            source: fact.source.clone(),
        });
    }
    files.sort_by(|left, right| {
        file_kind_rank(&left.kind)
            .cmp(&file_kind_rank(&right.kind))
            .then_with(|| left.path.cmp(&right.path))
    });
    files.dedup_by(|left, right| left.kind == right.kind && left.path == right.path);
    files
}

pub fn runtime_requirements_from_facts(facts: &[DetectedFact]) -> Vec<RuntimeRequirement> {
    let mut requirements = facts
        .iter()
        .filter(|fact| fact.kind == "runtime")
        .map(|fact| {
            let (ecosystem, constraint) = split_runtime_value(&fact.value);
            RuntimeRequirement {
                label: runtime_label(&ecosystem),
                ecosystem,
                constraint: constraint.filter(|value| !value.is_empty()),
                source: Some(fact.source.clone()),
            }
        })
        .collect::<Vec<_>>();
    requirements.sort_by(|left, right| left.ecosystem.cmp(&right.ecosystem));
    requirements.dedup_by(|left, right| {
        left.ecosystem == right.ecosystem && left.constraint == right.constraint
    });
    requirements
}

pub fn inspect_environment(detail: &ProjectDetail) -> EnvironmentInspection {
    let files = if detail.project_files.is_empty() {
        project_files_from_facts(&detail.facts)
    } else {
        detail.project_files.clone()
    };
    let requirements = if detail.runtime_requirements.is_empty() {
        runtime_requirements_from_facts(&detail.facts)
    } else {
        detail.runtime_requirements.clone()
    };
    let mut requirements = requirements;
    for inferred in inferred_runtimes(&detail.project.languages, &detail.project.package_managers) {
        if !requirements
            .iter()
            .any(|item| item.ecosystem == inferred.ecosystem)
        {
            requirements.push(inferred);
        }
    }
    let mut runtimes = inspect_runtimes(requirements);
    runtimes.sort_by(|left, right| left.ecosystem.cmp(&right.ecosystem));
    EnvironmentInspection {
        project_id: detail.project.id.clone(),
        runtimes,
        files,
    }
}

pub fn read_detected_file(
    project_root: &Path,
    relative_path: &str,
    allowed: &[ProjectFile],
) -> Result<ReadmeDocument> {
    // Do not resolve the path and then open it by name.  A project can be
    // changed by another process between those two operations (most notably
    // by replacing a component with a symlink).  The secure opener walks the
    // path from an anchored directory handle on Unix and validates the final
    // handle on Windows, so the bytes we read are the bytes we authorized.
    let (root, relative, file) = validate_project_file(project_root, relative_path, allowed)?;
    let opened = open_project_file(&root, &relative, &file)?;
    let mut bytes = Vec::with_capacity(MAX_FILE_BYTES + 1);
    opened
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() > MAX_FILE_BYTES;
    bytes.truncate(MAX_FILE_BYTES);
    if bytes.contains(&0) {
        return Err(Error::msg("binary files cannot be previewed"));
    }
    Ok(ReadmeDocument {
        path: normalize_relative(relative_path),
        content: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
    })
}

pub fn resolve_project_file(
    project_root: &Path,
    relative_path: &str,
    allowed: &[ProjectFile],
) -> Result<PathBuf> {
    let (_, _, file) = validate_project_file(project_root, relative_path, allowed)?;
    Ok(file)
}

fn validate_project_file(
    project_root: &Path,
    relative_path: &str,
    allowed: &[ProjectFile],
) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(Error::msg("document path is outside the project"));
    }
    let normalized = normalize_relative(relative_path);
    if !allowed
        .iter()
        .any(|file| file.path.replace("\\", "/") == normalized)
    {
        return Err(Error::msg("unsupported project document"));
    }
    let root = paths::canonicalize(project_root)?;
    let file = paths::canonicalize(&root.join(relative))?;
    if !paths::is_within(&file, &root) || !file.is_file() {
        return Err(Error::msg("document path is outside the project"));
    }
    Ok((root, relative.to_path_buf(), file))
}

fn open_project_file(root: &Path, relative: &Path, _expected: &Path) -> Result<fs::File> {
    #[cfg(unix)]
    {
        secure_project_open::open(root, relative)
    }
    #[cfg(windows)]
    {
        secure_project_open::open(root, relative)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = root;
        let _ = relative;
        Ok(fs::File::open(_expected)?)
    }
}

// Unix has the primitives needed to make the authorization check and open
// one operation: each component is opened relative to an already-opened
// directory and O_NOFOLLOW is applied to every component.  This prevents an
// attacker from swapping either a parent directory or the final document for
// a symlink after canonicalization but before the read.
#[cfg(unix)]
mod secure_project_open {
    use super::{Error, Result};
    use std::ffi::CString;
    use std::fs::File;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Component, Path};

    const AT_FDCWD: i32 = -100;
    const O_RDONLY: i32 = 0;

    #[cfg(target_os = "macos")]
    const O_CLOEXEC: i32 = 0x0100_0000;
    #[cfg(target_os = "macos")]
    const O_DIRECTORY: i32 = 0x0010_0000;
    #[cfg(target_os = "macos")]
    const O_NOFOLLOW: i32 = 0x0000_0100;

    #[cfg(target_os = "linux")]
    const O_CLOEXEC: i32 = 0o2_000_000;
    #[cfg(target_os = "linux")]
    const O_DIRECTORY: i32 = 0o200_000;
    #[cfg(target_os = "linux")]
    const O_NOFOLLOW: i32 = 0o400_000;

    // The first-release targets are macOS and Windows. Keep the Unix module
    // buildable on Linux for CI and fail closed on another Unix where the
    // platform constants have not been audited yet.
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    const O_CLOEXEC: i32 = 0;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    const O_DIRECTORY: i32 = 0;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    const O_NOFOLLOW: i32 = 0;

    const DIRECTORY_FLAGS: i32 = O_RDONLY | O_CLOEXEC | O_DIRECTORY | O_NOFOLLOW;
    const FILE_FLAGS: i32 = O_RDONLY | O_CLOEXEC | O_NOFOLLOW;

    unsafe extern "C" {
        fn openat(dirfd: i32, pathname: *const std::ffi::c_char, flags: i32, ...) -> i32;
    }

    pub(super) fn open(root: &Path, relative: &Path) -> Result<File> {
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        return Err(Error::msg(
            "secure project file reads are unsupported on this platform",
        ));

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let mut directory = open_path(root, DIRECTORY_FLAGS)?;
            let components = relative
                .components()
                .filter_map(|component| match component {
                    Component::Normal(value) => Some(value),
                    Component::CurDir => None,
                    _ => None,
                })
                .collect::<Vec<_>>();
            let Some((last, parents)) = components.split_last() else {
                return Err(Error::msg("document path is outside the project"));
            };

            for component in parents {
                directory = open_child(directory.as_raw_fd(), component, DIRECTORY_FLAGS)?;
            }

            let file = open_child(directory.as_raw_fd(), last, FILE_FLAGS)?;
            if !file.metadata()?.is_file() {
                return Err(Error::msg("document path is outside the project"));
            }
            Ok(file)
        }
    }

    fn open_path(path: &Path, flags: i32) -> Result<File> {
        let name = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| Error::msg("document path contains an invalid NUL byte"))?;
        open_fd(AT_FDCWD, &name, flags)
    }

    fn open_child(
        parent: std::os::fd::RawFd,
        component: &std::ffi::OsStr,
        flags: i32,
    ) -> Result<File> {
        let name = CString::new(component.as_bytes())
            .map_err(|_| Error::msg("document path contains an invalid NUL byte"))?;
        open_fd(parent, &name, flags)
    }

    fn open_fd(parent: i32, name: &CString, flags: i32) -> Result<File> {
        // No O_CREAT flag is ever passed, so openat's variadic mode argument
        // is intentionally omitted. The resulting descriptor is owned by
        // File immediately and is closed on every error/return path.
        let descriptor = unsafe { openat(parent, name.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

// Windows does not expose openat-style directory descriptors through the
// standard library. CreateFileW pins the object selected by the path; the
// final handle path is then queried and compared with the canonical project
// root before any bytes are read. Thus a reparse/junction swap can at most
// cause a rejected handle, never an outside file to be returned to callers.
#[cfg(windows)]
mod secure_project_open {
    use super::{paths, Error, Result};
    use std::ffi::c_void;
    use std::fs::File;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::path::{Path, PathBuf};

    type Handle = *mut c_void;

    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    const OPEN_EXISTING: u32 = 3;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ByHandleFileInformation {
        attributes: u32,
        creation_time: FileTime,
        last_access_time: FileTime,
        last_write_time: FileTime,
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            file_name: *const u16,
            desired_access: u32,
            share_mode: u32,
            security_attributes: *mut c_void,
            creation_disposition: u32,
            flags_and_attributes: u32,
            template_file: Handle,
        ) -> Handle;
        fn GetFileInformationByHandle(
            file: Handle,
            information: *mut ByHandleFileInformation,
        ) -> i32;
        fn GetFinalPathNameByHandleW(file: Handle, path: *mut u16, length: u32, flags: u32) -> u32;
    }

    pub(super) fn open(root: &Path, relative: &Path) -> Result<File> {
        let candidate = root.join(relative);
        let wide = wide_path(&candidate);
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error().into());
        }

        let file = unsafe { File::from_raw_handle(handle) };
        let mut information = ByHandleFileInformation::default();
        let has_information =
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) };
        if has_information == 0 {
            return Err(io::Error::last_os_error().into());
        }
        if information.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(Error::msg("document path contains a symbolic link"));
        }
        if !file.metadata()?.is_file() {
            return Err(Error::msg("document path is outside the project"));
        }

        let opened_path = normalize_handle_path(&final_path(&file)?);
        let canonical_root = paths::canonicalize(root)?;
        if !paths::is_within(&opened_path, &canonical_root) {
            return Err(Error::msg("document path is outside the project"));
        }
        Ok(file)
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn final_path(file: &File) -> Result<String> {
        let mut capacity = 512usize;
        loop {
            let mut buffer = vec![0u16; capacity];
            let length = unsafe {
                GetFinalPathNameByHandleW(
                    file.as_raw_handle(),
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    0,
                )
            } as usize;
            if length == 0 {
                return Err(io::Error::last_os_error().into());
            }
            if length < buffer.len() {
                return Ok(String::from_utf16_lossy(&buffer[..length]));
            }
            if capacity >= 32 * 1024 {
                return Err(Error::msg("resolved project path is too long"));
            }
            capacity = (length + 1).min(32 * 1024);
        }
    }

    fn normalize_handle_path(path: &str) -> PathBuf {
        let path = path.strip_prefix("\\\\?\\").unwrap_or(path);
        if let Some(unc) = path.strip_prefix("UNC\\") {
            PathBuf::from(format!("\\\\{unc}"))
        } else {
            PathBuf::from(path)
        }
    }
}

pub fn read_project_icon(
    project_id: &str,
    project_root: &Path,
    facts: &[DetectedFact],
    languages: &[String],
) -> ProjectIcon {
    let fallback = languages
        .first()
        .map(|language| language.to_ascii_lowercase())
        .unwrap_or_else(|| "generic".into());
    let Some(asset) = facts.iter().find(|fact| fact.kind == "asset") else {
        return language_icon(project_id, &fallback);
    };
    match load_icon_bytes(project_root, &asset.value) {
        Ok((mime, bytes)) => ProjectIcon {
            project_id: project_id.into(),
            kind: "asset".into(),
            source: Some(asset.value.clone()),
            mime_type: Some(mime.clone()),
            data_url: Some(format!("data:{mime};base64,{}", encode_base64(&bytes))),
        },
        Err(_) => language_icon(project_id, &fallback),
    }
}

fn language_icon(project_id: &str, language: &str) -> ProjectIcon {
    ProjectIcon {
        project_id: project_id.into(),
        kind: "language".into(),
        source: Some(language.into()),
        mime_type: None,
        data_url: None,
    }
}

pub(crate) fn project_icon_from_bytes(
    project_id: &str,
    mime: &str,
    bytes: &[u8],
    source: Option<String>,
) -> ProjectIcon {
    ProjectIcon {
        project_id: project_id.into(),
        kind: "override".into(),
        source,
        mime_type: Some(mime.into()),
        data_url: Some(format!("data:{mime};base64,{}", encode_base64(bytes))),
    }
}

pub(crate) fn load_icon_override(path: &Path) -> Result<(String, Vec<u8>)> {
    let file = paths::canonicalize(path)?;
    if !file.is_file() {
        return Err(Error::msg("icon path is not a file"));
    }
    let meta = file.metadata()?;
    if meta.len() == 0 || meta.len() > MAX_ICON_BYTES {
        return Err(Error::msg("icon file is empty or larger than 512 KB"));
    }
    let bytes = fs::read(&file)?;
    let mime = validate_icon_payload(None, &bytes)?;
    Ok((mime, bytes))
}

pub(crate) fn validate_icon_payload(declared_mime: Option<&str>, bytes: &[u8]) -> Result<String> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_ICON_BYTES {
        return Err(Error::msg("icon file is empty or larger than 512 KB"));
    }
    let mime = sniff_image_mime(bytes).ok_or_else(|| Error::msg("unsupported icon format"))?;
    if declared_mime.is_some_and(|declared| declared != mime) {
        return Err(Error::msg("icon MIME type does not match its content"));
    }
    validate_icon_dimensions(mime, bytes)?;
    Ok(mime.into())
}

fn load_icon_bytes(project_root: &Path, relative: &str) -> Result<(String, Vec<u8>)> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(Error::msg("icon path is outside the project"));
    }
    let root = paths::canonicalize(project_root)?;
    let file = paths::canonicalize(&root.join(relative))?;
    if !paths::is_within(&file, &root) || !file.is_file() {
        return Err(Error::msg("icon path is outside the project"));
    }
    // Read the icon through the same descriptor-based boundary as project
    // documents. Detected icon facts come from project files and must not be
    // able to escape through a symlink swap between canonicalization and the
    // actual read.
    let opened = open_project_file(&root, relative, &file)?;
    let meta = opened.metadata()?;
    if meta.len() == 0 || meta.len() > MAX_ICON_BYTES {
        return Err(Error::msg("icon file is too large"));
    }
    let mut bytes = Vec::with_capacity(meta.len().min(MAX_ICON_BYTES) as usize + 1);
    opened.take(MAX_ICON_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return Err(Error::msg("icon file is too large"));
    }
    let mime = validate_icon_payload(None, &bytes)?;
    Ok((mime, bytes))
}

fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.len() >= 4
        && (bytes.starts_with(&[0, 0, 1, 0]) || bytes.starts_with(&[0, 0, 2, 0]))
    {
        Some("image/x-icon")
    } else {
        None
    }
}

fn validate_icon_dimensions(mime: &str, bytes: &[u8]) -> Result<()> {
    let (width, height) = match mime {
        "image/png" if bytes.len() >= 24 && &bytes[12..16] == b"IHDR" => (
            u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
            u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
        ),
        "image/jpeg" => jpeg_dimensions(bytes).ok_or_else(|| Error::msg("invalid JPEG icon"))?,
        "image/webp" => webp_dimensions(bytes).ok_or_else(|| Error::msg("invalid WebP icon"))?,
        "image/x-icon" if bytes.len() >= 22 && u16::from_le_bytes([bytes[4], bytes[5]]) > 0 => {
            let width = if bytes[6] == 0 {
                256
            } else {
                u32::from(bytes[6])
            };
            let height = if bytes[7] == 0 {
                256
            } else {
                u32::from(bytes[7])
            };
            (width, height)
        }
        _ => return Err(Error::msg("invalid icon image")),
    };
    if width == 0
        || height == 0
        || width > MAX_ICON_DIMENSION
        || height > MAX_ICON_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_ICON_PIXELS
    {
        return Err(Error::msg("icon dimensions exceed 2048 px or 4 megapixels"));
    }
    Ok(())
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut index = 2usize;
    while index + 4 < bytes.len() {
        if bytes[index] != 0xFF {
            index += 1;
            continue;
        }
        while index < bytes.len() && bytes[index] == 0xFF {
            index += 1;
        }
        let marker = *bytes.get(index)?;
        index += 1;
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        let length = u16::from_be_bytes([*bytes.get(index)?, *bytes.get(index + 1)?]) as usize;
        if length < 2 || index + length > bytes.len() {
            return None;
        }
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) && length >= 7 {
            let height = u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as u32;
            return Some((width, height));
        }
        index += length;
    }
    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let kind = bytes.get(12..16)?;
    if kind == b"VP8X" && bytes.len() >= 30 {
        let width =
            1 + u32::from(bytes[24]) + (u32::from(bytes[25]) << 8) + (u32::from(bytes[26]) << 16);
        let height =
            1 + u32::from(bytes[27]) + (u32::from(bytes[28]) << 8) + (u32::from(bytes[29]) << 16);
        Some((width, height))
    } else if kind == b"VP8L" && bytes.len() >= 25 && bytes[20] == 0x2F {
        let width = 1 + u32::from(bytes[21]) + ((u32::from(bytes[22]) & 0x3F) << 8);
        let height = 1
            + (u32::from(bytes[22]) >> 6)
            + (u32::from(bytes[23]) << 2)
            + ((u32::from(bytes[24]) & 0x0F) << 10);
        Some((width, height))
    } else if kind == b"VP8 " && bytes.len() >= 30 && bytes[23..26] == [0x9D, 0x01, 0x2A] {
        let width = u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF;
        let height = u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF;
        Some((u32::from(width), u32::from(height)))
    } else {
        None
    }
}

fn inspect_runtime(requirement: RuntimeRequirement) -> RuntimeStatus {
    let local_version = probe_local_version(&requirement.ecosystem);
    let match_state = match (&requirement.constraint, &local_version) {
        (None, _) => "undeclared".into(),
        (Some(constraint), Some(local)) => match_constraint(constraint, local),
        (Some(_), None) => "missing".into(),
    };
    RuntimeStatus {
        ecosystem: requirement.ecosystem,
        label: requirement.label,
        constraint: requirement.constraint,
        source: requirement.source,
        local_version,
        match_state,
    }
}

/// Probe several fixed, allow-listed runtime programs in parallel while
/// keeping the amount of process work bounded.  Environment inspection is a
/// read-only operation, so a small worker fan-out makes a project with many
/// declared runtimes responsive without creating one thread per row.
fn inspect_runtimes(requirements: Vec<RuntimeRequirement>) -> Vec<RuntimeStatus> {
    if requirements.len() < 2 {
        return requirements.into_iter().map(inspect_runtime).collect();
    }

    let worker_count = requirements.len().min(MAX_VERSION_PROBES);
    let chunk_size = requirements.len().div_ceil(worker_count);
    let mut runtimes = Vec::with_capacity(requirements.len());

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for chunk in requirements.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .cloned()
                    .map(inspect_runtime)
                    .collect::<Vec<_>>()
            }));
        }
        for handle in handles {
            if let Ok(chunk) = handle.join() {
                runtimes.extend(chunk);
            }
        }
    });
    runtimes
}

fn inferred_runtimes(languages: &[String], package_managers: &[String]) -> Vec<RuntimeRequirement> {
    let mut runtimes = Vec::new();
    let mut push = |ecosystem: &str| {
        if !runtimes
            .iter()
            .any(|item: &RuntimeRequirement| item.ecosystem == ecosystem)
        {
            runtimes.push(RuntimeRequirement {
                ecosystem: ecosystem.into(),
                label: runtime_label(ecosystem),
                constraint: None,
                source: None,
            });
        }
    };
    for language in languages {
        match language.as_str() {
            "JavaScript" | "TypeScript" => push("node"),
            "Python" => push("python"),
            "Rust" => push("rust"),
            "Java" | "Kotlin" => push("java"),
            "Go" => push("go"),
            "C#" | "F#" | "Visual Basic" => push("dotnet"),
            "Dart" => push("dart"),
            _ => {}
        }
    }
    if package_managers.iter().any(|item| item == "pub") {
        push("flutter");
    }
    runtimes
}

fn probe_local_version(ecosystem: &str) -> Option<String> {
    let (program, args) = match ecosystem {
        "node" => ("node", vec!["--version"]),
        "python" => ("python", vec!["--version"]),
        "rust" => ("rustc", vec!["--version"]),
        "java" => ("java", vec!["-version"]),
        "go" => ("go", vec!["version"]),
        "dotnet" => ("dotnet", vec!["--version"]),
        "dart" => ("dart", vec!["--version"]),
        "flutter" => ("flutter", vec!["--version"]),
        _ => return None,
    };
    if let Ok(cache) = version_cache().lock() {
        if let Some(entry) = cache.get(ecosystem) {
            if entry.observed_at.elapsed() < VERSION_CACHE_TTL {
                return entry.version.clone();
            }
        }
    }
    let deadline = Instant::now() + VERSION_TIMEOUT;
    let version = run_version_command_until(program, &args, deadline).or_else(|| {
        if ecosystem == "python" {
            run_version_command_until("python3", &args, deadline)
        } else {
            None
        }
    });
    if let Ok(mut cache) = version_cache().lock() {
        cache.insert(
            ecosystem.into(),
            CachedVersion {
                observed_at: Instant::now(),
                version: version.clone(),
            },
        );
    }
    version
}

#[cfg(test)]
fn run_version_command(program: &str, args: &[&str]) -> Option<String> {
    run_version_command_until(program, args, Instant::now() + VERSION_TIMEOUT)
}

fn run_version_command_until(program: &str, args: &[&str], deadline: Instant) -> Option<String> {
    if Instant::now() >= deadline {
        return None;
    }
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    // Drain both streams concurrently.  A child can otherwise block itself
    // by filling one pipe before it exits, which would make the timeout
    // ineffective for a noisy or misbehaving version command.
    let stdout = child.stdout.take().map(read_pipe_async);
    let stderr = child.stderr.take().map(read_pipe_async);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }?;

    let stdout = receive_pipe(stdout, deadline)?;
    let stderr = receive_pipe(stderr, deadline)?;
    if !status.success() {
        return None;
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    parse_version(&combined)
}

fn read_pipe_async<T>(stream: T) -> mpsc::Receiver<Vec<u8>>
where
    T: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stream
            .take(MAX_VERSION_OUTPUT_BYTES)
            .read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });
    receiver
}

fn receive_pipe(receiver: Option<mpsc::Receiver<Vec<u8>>>, deadline: Instant) -> Option<Vec<u8>> {
    let receiver = receiver?;
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok()
}

fn parse_version(raw: &str) -> Option<String> {
    let text = raw.lines().find(|line| !line.trim().is_empty())?.trim();
    let mut version = String::new();
    let mut started = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            started = true;
            version.push(ch);
        } else if started && (ch == '.' || ch == '-') {
            version.push(ch);
        } else if started {
            break;
        }
    }
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

fn match_constraint(constraint: &str, local: &str) -> String {
    let Some(local_version) = parse_semver(local) else {
        return "unknown".into();
    };
    let Some(ok) = constraint_matches(constraint, local_version) else {
        return "unknown".into();
    };
    if ok {
        "match".into()
    } else {
        "mismatch".into()
    }
}

fn constraint_matches(constraint: &str, local: (u64, u64, u64)) -> Option<bool> {
    let trimmed = constraint.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Some(true);
    }
    // We deliberately do not guess at disjunctions. Returning `None` makes
    // the UI show "无法判定" instead of claiming a match for a range whose
    // semantics we do not implement.
    if trimmed.contains("||") {
        return None;
    }
    // Handle conjunctions before single-operator parsing. A range such as
    // ">=18 <20" must evaluate both bounds; parsing the first operator only
    // would silently ignore the upper bound.
    if trimmed.contains(',') || trimmed.chars().any(char::is_whitespace) {
        let parts = trimmed
            .split(|character: char| character == ',' || character.is_whitespace())
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return None;
        }
        let mut matches = true;
        for part in parts {
            matches &= constraint_matches(part, local)?;
        }
        return Some(matches);
    }
    if let Some(rest) = trimmed.strip_prefix(">=") {
        return parse_semver(rest)
            .map(|bound| cmp_semver(local, bound) != std::cmp::Ordering::Less);
    }
    if let Some(rest) = trimmed.strip_prefix("<=") {
        return parse_semver(rest)
            .map(|bound| cmp_semver(local, bound) != std::cmp::Ordering::Greater);
    }
    if let Some(rest) = trimmed.strip_prefix('>') {
        return parse_semver(rest)
            .map(|bound| cmp_semver(local, bound) == std::cmp::Ordering::Greater);
    }
    if let Some(rest) = trimmed.strip_prefix('<') {
        return parse_semver(rest)
            .map(|bound| cmp_semver(local, bound) == std::cmp::Ordering::Less);
    }
    if let Some(rest) = trimmed.strip_prefix('^') {
        let bound = parse_semver(rest)?;
        let next = if bound.0 > 0 {
            (bound.0 + 1, 0, 0)
        } else if bound.1 > 0 {
            (0, bound.1 + 1, 0)
        } else {
            (0, 0, bound.2 + 1)
        };
        return Some(
            cmp_semver(local, bound) != std::cmp::Ordering::Less
                && cmp_semver(local, next) == std::cmp::Ordering::Less,
        );
    }
    if let Some(rest) = trimmed.strip_prefix('~') {
        let bound = parse_semver(rest)?;
        let next = (bound.0, bound.1 + 1, 0);
        return Some(
            cmp_semver(local, bound) != std::cmp::Ordering::Less
                && cmp_semver(local, next) == std::cmp::Ordering::Less,
        );
    }
    if let Some(rest) = trimmed.strip_prefix('=') {
        return parse_semver(rest).map(|bound| local == bound);
    }
    parse_semver(trimmed).map(|bound| {
        if bound.2 == 0 && bound.1 == 0 {
            local.0 == bound.0
        } else if bound.2 == 0 {
            local.0 == bound.0 && local.1 == bound.1
        } else {
            local == bound
        }
    })
}

fn parse_semver(value: &str) -> Option<(u64, u64, u64)> {
    let cleaned = value.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = cleaned
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

fn cmp_semver(left: (u64, u64, u64), right: (u64, u64, u64)) -> std::cmp::Ordering {
    left.0
        .cmp(&right.0)
        .then(left.1.cmp(&right.1))
        .then(left.2.cmp(&right.2))
}

fn split_runtime_value(value: &str) -> (String, Option<String>) {
    match value.split_once('@') {
        Some((ecosystem, constraint)) => (ecosystem.to_string(), Some(constraint.to_string())),
        None => (value.to_string(), None),
    }
}

fn runtime_source_path(source: &str) -> Option<String> {
    source
        .split('#')
        .next()
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn runtime_label(ecosystem: &str) -> String {
    match ecosystem {
        "node" => "Node.js".into(),
        "python" => "Python".into(),
        "rust" => "Rust".into(),
        "java" => "Java".into(),
        "go" => "Go".into(),
        "dotnet" => ".NET".into(),
        "dart" => "Dart".into(),
        "flutter" => "Flutter".into(),
        other => other.into(),
    }
}

fn file_kind_rank(kind: &str) -> usize {
    match kind {
        "manifest" => 0,
        "packageManager" => 1,
        "lockfile" => 2,
        "runtime" => 3,
        _ => 4,
    }
}

fn normalize_relative(path: &str) -> String {
    Path::new(path)
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as usize;
        let b = chunk.get(1).copied().unwrap_or(0) as usize;
        let c = chunk.get(2).copied().unwrap_or(0) as usize;
        let triple = (a << 16) | (b << 8) | c;
        out.push(TABLE[(triple >> 18) & 63] as char);
        out.push(TABLE[(triple >> 12) & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[triple & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        constraint_matches, parse_semver, parse_version, read_detected_file, resolve_project_file,
        run_version_command,
    };
    use crate::models::ProjectFile;
    use std::fs;
    use std::time::{Duration, Instant};

    #[test]
    fn parses_common_version_strings() {
        assert_eq!(parse_version("v18.20.4").as_deref(), Some("18.20.4"));
        assert_eq!(
            parse_version("go version go1.22.3 windows/amd64").as_deref(),
            Some("1.22.3")
        );
        assert_eq!(parse_version("Python 3.12.1").as_deref(), Some("3.12.1"));
    }

    #[test]
    fn matches_caret_and_exact_constraints() {
        let local = parse_semver("18.20.4").unwrap();
        assert_eq!(constraint_matches("^18.0.0", local), Some(true));
        assert_eq!(constraint_matches("^19.0.0", local), Some(false));
        assert_eq!(constraint_matches(">=18", local), Some(true));
        assert_eq!(constraint_matches("18", local), Some(true));
        assert_eq!(constraint_matches(">=18 <20", local), Some(true));
        assert_eq!(
            constraint_matches(">=18 <20", parse_semver("20.0.0").unwrap()),
            Some(false)
        );
        assert_eq!(
            constraint_matches(">=18,<20", parse_semver("20.0.0").unwrap()),
            Some(false)
        );
        assert_eq!(constraint_matches(">=20 || <16", local), None);
    }

    #[test]
    fn reads_an_allowed_file_without_following_an_external_symlink() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.md"), "outside").unwrap();
        fs::write(project.path().join("README.md"), "inside").unwrap();

        let allowed = [ProjectFile {
            kind: "manifest".into(),
            path: "README.md".into(),
            source: "README.md".into(),
        }];
        let document = read_detected_file(project.path(), "README.md", &allowed).unwrap();
        assert_eq!(document.content, "inside");

        #[cfg(unix)]
        {
            fs::remove_file(project.path().join("README.md")).unwrap();
            std::os::unix::fs::symlink(
                outside.path().join("secret.md"),
                project.path().join("README.md"),
            )
            .unwrap();
            assert!(read_detected_file(project.path(), "README.md", &allowed).is_err());

            fs::remove_file(project.path().join("README.md")).unwrap();
            fs::write(project.path().join("inside.md"), "inside target").unwrap();
            std::os::unix::fs::symlink("inside.md", project.path().join("README.md")).unwrap();
            // The target is still under the project, but a detected document
            // must be a regular path component rather than a symlink. This is
            // the case that canonicalize-plus-File::open would otherwise
            // silently follow.
            assert!(read_detected_file(project.path(), "README.md", &allowed).is_err());
        }
        #[cfg(windows)]
        {
            fs::remove_file(project.path().join("README.md")).unwrap();
            // Symlink creation can be disabled on older Windows setups. If
            // the host grants the privilege, exercise the same outside-root
            // rejection used by the desktop/MCP file reader.
            if std::os::windows::fs::symlink_file(
                outside.path().join("secret.md"),
                project.path().join("README.md"),
            )
            .is_ok()
            {
                assert!(read_detected_file(project.path(), "README.md", &allowed).is_err());
            }
        }
    }

    #[test]
    fn rejects_parent_and_rooted_document_paths_before_filesystem_access() {
        let project = tempfile::tempdir().unwrap();
        let allowed = [ProjectFile {
            kind: "manifest".into(),
            path: "README.md".into(),
            source: "README.md".into(),
        }];
        for path in ["../README.md", "./../README.md", "/tmp/README.md"] {
            assert!(
                resolve_project_file(project.path(), path, &allowed).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn version_probe_terminates_a_hanging_process() {
        let started = Instant::now();
        let version = run_version_command("sh", &["-c", "sleep 10"]);

        assert!(version.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "probe exceeded its two-second deadline"
        );
    }

    #[test]
    #[cfg(windows)]
    fn version_probe_terminates_a_hanging_process() {
        let started = Instant::now();
        let version = run_version_command(
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 10",
            ],
        );

        assert!(version.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "probe exceeded its two-second deadline"
        );
    }
}
