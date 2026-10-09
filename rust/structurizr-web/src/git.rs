//! Reading past versions of a workspace file out of git.
//!
//! Everything here shells out to the `git` binary rather than linking a git
//! implementation: the server only needs the history of one file and the
//! contents of a blob, `git` is already installed wherever a workspace is
//! version-controlled, and a library would add a large dependency for two
//! commands.
//!
//! No command here writes anything — not to the repository, not to the working
//! tree, not to a temporary file next to the workspace. A comparison is a read.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

/// The revision name for "the file as it is on disk right now", including
/// uncommitted edits. Not a git revision — the callers special-case it.
pub const WORKING: &str = "working";

/// One commit that touched the workspace file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    /// Full commit sha, the stable identifier a link should carry.
    pub sha: String,
    /// Abbreviated sha, for display.
    pub short: String,
    pub author: String,
    /// Author date, ISO 8601 with offset.
    pub date: String,
    /// First line of the commit message.
    pub subject: String,
}

/// The history of one workspace file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    /// Path of the workspace file relative to the repository root — the path
    /// `git show <rev>:<path>` needs.
    pub file: String,
    /// Commits that touched the file, newest first.
    pub revisions: Vec<Revision>,
    /// True when the working tree copy differs from `HEAD`, i.e. when
    /// comparing against [`WORKING`] shows something the history does not.
    pub dirty: bool,
}

/// Where a workspace file sits inside a repository.
struct Located {
    /// Directory to run git in (the file's own directory).
    dir: PathBuf,
    /// File name, used with `git show <rev>:./<name>`.
    name: String,
    /// Path relative to the repository root.
    relative: String,
}

fn locate(path: &Path) -> Result<Located> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("workspace path has no file name: {}", path.display()))?
        .to_string();

    // `--show-prefix` is the path of `dir` below the repository root, and
    // doubles as the "is this a repository at all" check.
    let prefix = git(&dir, &["rev-parse", "--show-prefix"])
        .context("not inside a git repository")?
        .trim()
        .to_string();

    Ok(Located {
        relative: format!("{prefix}{name}"),
        dir,
        name,
    })
}

/// True when `path` lives inside a git work tree — the cheap check that decides
/// whether to offer the comparison page at all.
pub fn is_tracked(path: &Path) -> bool {
    let Ok(loc) = locate(path) else {
        return false;
    };
    // A file in a repository but never committed has no history to compare.
    git(&loc.dir, &["ls-files", "--error-unmatch", "--", &loc.name]).is_ok()
}

/// Every file the workspace is made of, repository-root-relative: the entry
/// file first, then the files it `!include`s, transitively.
///
/// Read from the working tree rather than from git, and deliberately
/// forgiving — a file that cannot be read is simply not part of the set, so a
/// broken include costs a few history entries rather than the whole page.
fn workspace_files(loc: &Located) -> Vec<String> {
    let mut files = vec![loc.relative.clone()];
    collect_includes(&loc.dir.join(&loc.name), &loc.relative, &mut files, 0);
    files
}

fn collect_includes(file: &Path, relative: &str, out: &mut Vec<String>, depth: usize) {
    const MAX_DEPTH: usize = 16;
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(source) = std::fs::read_to_string(file) else {
        return;
    };
    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("!include ")
            .or_else(|| trimmed.strip_prefix("!INCLUDE "))
        else {
            continue;
        };
        let rel = rest.trim().trim_matches('"');
        let included = join_repo_path(relative, rel);
        if out.contains(&included) {
            continue; // already collected, or a cycle
        }
        out.push(included.clone());
        let Some(dir) = file.parent() else { continue };
        collect_includes(&dir.join(rel), &included, out, depth + 1);
    }
}

/// The commits that touched the workspace, newest first, at most `limit` of
/// them.
///
/// "The workspace" is the entry file *and every file it `!include`s*: a
/// multi-file workspace usually keeps the model in the included files, so a
/// history of the entry file alone would list almost no commits and hide every
/// interesting change. The include set is read from the working tree, which is
/// the workspace's current shape — the pickers only need candidate revisions,
/// and each revision's own include set is resolved when it is read.
///
/// `--follow` (history across a rename) only works for a single path, so it is
/// used for the single-file case and dropped for the multi-file one, where git
/// itself refuses it.
pub fn history(path: &Path, limit: usize) -> Result<History> {
    let loc = locate(path)?;
    let files = workspace_files(&loc);

    // %x1f (unit separator) cannot appear in a sha, a name or a subject line,
    // so no quoting or escaping is needed on either side.
    let format = "--format=%H%x1f%h%x1f%an%x1f%aI%x1f%s";
    let max = format!("--max-count={limit}");
    let mut args: Vec<&str> = vec!["log", &max, format];
    if files.len() == 1 {
        args.push("--follow");
    }
    args.push("--");
    // Pathspecs are relative to the directory git runs in, which is the
    // workspace file's own; `:(top)` re-anchors them at the repository root, so
    // an include that sits above that directory still resolves.
    let pathspecs: Vec<String> = files.iter().map(|f| format!(":(top){f}")).collect();
    args.extend(pathspecs.iter().map(String::as_str));
    let out = git(&loc.dir, &args)?;

    let revisions = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            Some(Revision {
                sha: parts.next()?.to_string(),
                short: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect();

    // `diff --quiet` exits 1 when there is a difference, which `git()` reports
    // as an error; here a non-zero exit is the answer, not a failure.
    let mut diff_args: Vec<&str> = vec!["diff", "--quiet", "HEAD", "--"];
    diff_args.extend(pathspecs.iter().map(String::as_str));
    let dirty = git(&loc.dir, &diff_args).is_err();

    Ok(History {
        file: loc.relative,
        revisions,
        dirty,
    })
}

/// The contents of the workspace file at `rev`, with `!include`d files spliced
/// in **from the same revision**.
///
/// The splicing is done here rather than by the DSL parser because the parser
/// resolves includes against the filesystem, which holds today's copy of the
/// included file. Reading one revision's entry file and another revision's
/// includes would produce a model that never existed.
pub fn read(path: &Path, rev: &str) -> Result<String> {
    let loc = locate(path)?;
    let source = show(&loc.dir, rev, &format!("./{}", loc.name))?;
    splice_includes(&loc.dir, rev, &loc.relative, &source, 0)
}

/// Resolve a revision to its full sha, so a page built from `HEAD` records
/// which commit that actually was.
pub fn resolve(path: &Path, rev: &str) -> Result<String> {
    let loc = locate(path)?;
    Ok(git(&loc.dir, &["rev-parse", rev])?.trim().to_string())
}

/// `git show <rev>:<path>` — `path` is relative to the current directory when
/// it starts with `./`, and to the repository root otherwise.
fn show(dir: &Path, rev: &str, path: &str) -> Result<String> {
    git(dir, &["show", &format!("{rev}:{path}")]).with_context(|| {
        format!("{path} does not exist at revision {rev} (or {rev} is not a revision)")
    })
}

/// Splice `!include <path>` lines with the included file's contents *at the
/// same revision*, mirroring the DSL parser's own preprocessing (paths are
/// relative to the including file, with a depth cap against cycles).
fn splice_includes(
    dir: &Path,
    rev: &str,
    including_file: &str,
    source: &str,
    depth: usize,
) -> Result<String> {
    const MAX_DEPTH: usize = 16;
    if depth > MAX_DEPTH {
        bail!("!include depth exceeded (cycle?) while reading revision {rev}");
    }

    // Nothing to do for the overwhelmingly common single-file workspace.
    if !source.contains("!include") && !source.contains("!INCLUDE") {
        return Ok(source.to_string());
    }

    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim_start();
        let rest = trimmed
            .strip_prefix("!include ")
            .or_else(|| trimmed.strip_prefix("!INCLUDE "));
        match rest {
            Some(rest) => {
                let rel = rest.trim().trim_matches('"');
                let included = join_repo_path(including_file, rel);
                let content = show(dir, rev, &included)?;
                out.push_str(&splice_includes(dir, rev, &included, &content, depth + 1)?);
                out.push('\n');
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    Ok(out)
}

/// Resolve `rel` against the directory of `including_file`, both being
/// repository-root-relative paths, normalising `.` and `..` textually — git
/// blob paths have no filesystem to canonicalise against.
fn join_repo_path(including_file: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = including_file.split('/').collect();
    parts.pop(); // the including file's own name
    for segment in rel.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Run git in `dir`, returning stdout on a zero exit and stderr as the error
/// message otherwise.
fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("cannot run git (is it installed?): git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(if stderr.is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            stderr
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_paths_resolve_against_the_including_file() {
        assert_eq!(
            join_repo_path("docs/workspace.dsl", "model.dsl"),
            "docs/model.dsl"
        );
        assert_eq!(
            join_repo_path("docs/workspace.dsl", "./model.dsl"),
            "docs/model.dsl"
        );
        assert_eq!(
            join_repo_path("docs/workspace.dsl", "../model.dsl"),
            "model.dsl"
        );
        assert_eq!(
            join_repo_path("a/b/workspace.dsl", "../c/model.dsl"),
            "a/c/model.dsl"
        );
        assert_eq!(
            join_repo_path("workspace.dsl", "parts/model.dsl"),
            "parts/model.dsl"
        );
    }

    /// The repository this test runs in is a git repository, which makes the
    /// happy paths testable without building a fixture repository.
    #[test]
    fn reads_a_committed_file_from_history() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(is_tracked(&path), "this crate's Cargo.toml is committed");

        let history = history(&path, 5).expect("history");
        assert!(!history.revisions.is_empty());
        assert!(history.file.ends_with("structurizr-web/Cargo.toml"));

        let newest = &history.revisions[0].sha;
        let content = read(&path, newest).expect("blob");
        assert!(content.contains("name = \"structurizr-web\""));
    }

    #[test]
    fn an_untracked_path_is_reported_as_such() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("no-such-workspace.dsl");
        assert!(!is_tracked(&path));
    }

    /// A multi-file workspace in a throwaway repository: the history must
    /// cover the included file, and reading a revision must splice *that*
    /// revision's includes rather than the working tree's.
    #[test]
    fn a_multi_file_workspace_is_followed_through_its_includes() {
        let dir =
            std::env::temp_dir().join(format!("structurizrx-git-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("parts")).expect("temp dir");

        let run = |args: &[&str]| {
            git(&dir, args).unwrap_or_else(|e| panic!("git {}: {e:#}", args.join(" ")));
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);

        let entry = dir.join("workspace.dsl");
        std::fs::write(&entry, "workspace {\n!include parts/model.dsl\n}\n").unwrap();
        std::fs::write(dir.join("parts/model.dsl"), "model {\n}\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-qm", "first"]);

        // A commit that touches only the included file.
        std::fs::write(dir.join("parts/model.dsl"), "model {\n// second\n}\n").unwrap();
        run(&["commit", "-qam", "second"]);

        let log = history(&entry, 10).expect("history");
        let subjects: Vec<&str> = log.revisions.iter().map(|r| r.subject.as_str()).collect();
        assert_eq!(
            subjects,
            vec!["second", "first"],
            "includes are part of the history"
        );
        assert!(!log.dirty);

        let oldest = &log.revisions[1].sha;
        let source = read(&entry, oldest).expect("blob");
        assert!(source.contains("model {"), "the include was spliced in");
        assert!(
            !source.contains("// second"),
            "the include came from the same revision, not the working tree"
        );

        std::fs::write(dir.join("parts/model.dsl"), "model {\n// third\n}\n").unwrap();
        assert!(
            history(&entry, 10).expect("history").dirty,
            "an edited include is a dirty tree"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_bad_revision_is_an_error_naming_the_revision() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let err = read(&path, "definitely-not-a-rev").expect_err("must fail");
        assert!(
            err.to_string().contains("definitely-not-a-rev")
                || err
                    .chain()
                    .any(|c| c.to_string().contains("definitely-not-a-rev")),
            "error should name the revision: {err:#}"
        );
    }
}
