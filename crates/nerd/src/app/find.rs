use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SearchKind {
    #[default]
    All,
    Files,
    Folders,
}

impl SearchKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            SearchKind::All => "All",
            SearchKind::Files => "Files",
            SearchKind::Folders => "Folders",
        }
    }
}

#[derive(Default)]
pub(super) struct FileSearch {
    pub(super) open: bool,
    pub(super) focus: bool,
    pub(super) query: String,
    pub(super) kind: SearchKind,
    pub(super) submitted: String,
    pub(super) submitted_kind: SearchKind,
    pub(super) hits: Vec<String>,
    pub(super) selected: usize,
    pub(super) pending: bool,
    job: Option<std::sync::mpsc::Receiver<(String, SearchKind, Vec<String>)>>,
}

impl FileSearch {
    pub(super) fn sync(&mut self, ctx: &Context) {
        let query = self.query.trim().to_owned();
        if query == self.submitted && self.kind == self.submitted_kind {
            return;
        }
        self.submitted = query.clone();
        self.submitted_kind = self.kind;
        let kind = self.kind;
        self.hits.clear();
        self.selected = 0;
        if query.chars().count() < 2 {
            self.pending = false;
            self.job = None;
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let _ = std::thread::Builder::new()
            .name("crawn-find-path".into())
            .spawn(move || {
                let _ = tx.send((query.clone(), kind, find_paths(&query, kind)));
                ctx.request_repaint();
            });
        self.pending = true;
        self.job = Some(rx);
    }

    pub(super) fn poll(&mut self) {
        let ready = self.job.as_ref().and_then(|job| job.try_recv().ok());
        if let Some((query, kind, hits)) = ready {
            self.job = None;
            self.pending = false;
            if query == self.query.trim() && kind == self.kind {
                self.hits = hits;
                self.selected = 0;
            }
        }
    }
}

fn find_paths(query: &str, kind: SearchKind) -> Vec<String> {
    let mut hits = Vec::new();
    #[cfg(target_os = "macos")]
    if let Ok(output) = std::process::Command::new("/usr/bin/mdfind")
        .args(["-name", query])
        .output()
    {
        hits.extend(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .take(200)
                .map(str::to_owned),
        );
    }
    if query.split_whitespace().nth(1).is_some() {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            phrase_paths(&std::path::PathBuf::from(home), query, kind, &mut hits, &mut 0, 0);
        }
    }
    if hits.is_empty() {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            walk_paths(&std::path::PathBuf::from(home), query, kind, &mut hits, 0);
        }
    }
    let phrase = query.to_lowercase();
    hits.sort_by(|left, right| name_has_phrase(right, &phrase).cmp(&name_has_phrase(left, &phrase)));
    hits.dedup();
    if kind != SearchKind::All {
        hits.retain(|path| {
            let folder = std::path::Path::new(path).is_dir();
            match kind {
                SearchKind::Folders => folder,
                SearchKind::Files => !folder,
                SearchKind::All => true,
            }
        });
    }
    hits.truncate(40);
    hits
}

pub(super) fn home_path(path: &str) -> String {
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return path.to_owned();
    };
    let home = home.to_string_lossy();
    path.strip_prefix(home.as_ref())
        .map(|rest| format!("~{rest}"))
        .unwrap_or_else(|| path.to_owned())
}

fn name_has_phrase(path: &str, phrase: &str) -> i32 {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if name.to_lowercase().contains(phrase) { 1 } else { 0 }
}

fn phrase_paths(
    dir: &std::path::Path,
    query: &str,
    kind: SearchKind,
    hits: &mut Vec<String>,
    found: &mut usize,
    depth: usize,
) {
    if depth > 4 || *found >= 40 {
        return;
    }
    let phrase = query.to_lowercase();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let raw = entry.file_name();
        let name = raw.to_string_lossy();
        if name.starts_with('.') || matches!(name.as_ref(), "node_modules" | "Library" | "target" | ".git") {
            continue;
        }
        let path = entry.path();
        let is_dir = path.is_dir();
        let allowed = match kind {
            SearchKind::All => true,
            SearchKind::Files => !is_dir,
            SearchKind::Folders => is_dir,
        };
        if allowed && name.to_lowercase().contains(&phrase) {
            hits.push(path.display().to_string());
            *found += 1;
        }
        if is_dir {
            phrase_paths(&path, query, kind, hits, found, depth + 1);
        }
    }
}

fn walk_paths(dir: &std::path::Path, query: &str, kind: SearchKind, hits: &mut Vec<String>, depth: usize) {
    if hits.len() >= 40 || depth > 4 {
        return;
    }
    let needle = query.to_lowercase();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if hits.len() >= 40 {
            return;
        }
        let raw = entry.file_name();
        let name = raw.to_string_lossy();
        if name.starts_with('.') || name == "node_modules" || name == "Library" {
            continue;
        }
        let path = entry.path();
        let is_dir = path.is_dir();
        let allowed = match kind {
            SearchKind::All => true,
            SearchKind::Files => !is_dir,
            SearchKind::Folders => is_dir,
        };
        if allowed && name.to_lowercase().contains(&needle) {
            hits.push(path.display().to_string());
        }
        if is_dir {
            walk_paths(&path, query, kind, hits, depth + 1);
        }
    }
}
