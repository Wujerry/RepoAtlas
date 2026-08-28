use crate::detect;
use crate::error::Result;
use crate::models::ScanProgress;
use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub path: PathBuf,
}

pub struct ScanEngine<'a> {
    pub scan_id: String,
    pub root: PathBuf,
    pub cancel: &'a AtomicBool,
    pub on_progress: &'a dyn Fn(ScanProgress),
}

impl<'a> ScanEngine<'a> {
    pub fn walk(&self) -> Result<(Vec<DiscoveredProject>, u64, Vec<String>, bool)> {
        let mut discovered = Vec::new();
        let mut errors = Vec::new();
        let mut visited = 0_u64;
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok((discovered, visited, errors, true));
            }
            visited += 1;
            if visited == 1 || visited.is_multiple_of(32) {
                (self.on_progress)(ScanProgress {
                    scan_id: self.scan_id.clone(),
                    root_path: paths::path_to_string(&self.root),
                    phase: "walking".into(),
                    visited,
                    discovered: discovered.len() as u64,
                    current_path: Some(paths::path_to_string(&dir)),
                    message: None,
                });
            }
            if detect::is_project_root(&dir) {
                discovered.push(DiscoveredProject { path: dir });
                continue;
            }
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(err) => {
                    errors.push(format!("{}: {err}", paths::path_to_string(&dir)));
                    continue;
                }
            };
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(err) => {
                        errors.push(format!("{}: {err}", paths::path_to_string(&dir)));
                        continue;
                    }
                };
                let path = entry.path();
                let Ok(meta) = entry.metadata() else {
                    continue;
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if detect::is_skip_dir(&name) {
                        continue;
                    }
                    stack.push(path);
                }
            }
        }
        Ok((discovered, visited, errors, false))
    }
}

pub fn path_exists(path: &Path) -> bool {
    path.exists()
}
