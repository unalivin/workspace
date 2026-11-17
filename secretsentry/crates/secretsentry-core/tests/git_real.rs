use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;

use app_core::git::{self, Fetched, HistoryOptions, ObjectId, ObjectKind, Repository};
use app_core::testkit::{TempDir, sample_hosting_token};

const START: i64 = 1_700_000_000;

fn git_is_installed() -> bool {
    Command::new("git").arg("--version").output().map(|output| output.status.success()).unwrap_or(false)
}

fn raw(root: &Path, time: i64, arguments: &[&str]) -> Vec<u8> {
    let date = format!("{time} +0000");
    let output = Command::new("git")
        .current_dir(root)
        .args(["-c", "user.name=Sample Author", "-c", "user.email=author", "-c", "core.autocrlf=false"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false", "-c", "gc.auto=0"])
        .args(["-c", "init.defaultBranch=main", "-c", "core.fsmonitor=false", "-c", "advice.detachedHead=false"])
        .args(arguments)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .output()
        .expect("start git");
    assert!(output.status.success(), "git {arguments:?} failed: {}", String::from_utf8_lossy(&output.stderr));
    output.stdout
}

fn run(root: &Path, time: i64, arguments: &[&str]) -> String {
    String::from_utf8_lossy(&raw(root, time, arguments)).into_owned()
}

fn commit(root: &Path, time: i64, subject: &str) {
    run(root, time, &["add", "-A"]);
    run(root, time, &["commit", "-q", "-m", subject]);
}

fn notes(lines: usize) -> String {
    (1..=lines).map(|number| format!("note {number}: the quick brown fox jumps over the lazy dog\n")).collect()
}

fn build(root: &Path) {
    run(root, START, &["init", "-q"]);
    fs::create_dir_all(root.join("src/deep")).unwrap();
    fs::write(root.join("notes.txt"), notes(200)).unwrap();
    fs::write(root.join("src/app.txt"), "first version\n").unwrap();
    fs::write(root.join("src/deep/data.bin"), b"\0\x01\x02binary").unwrap();
    commit(root, START, "one");
    run(root, START, &["branch", "side"]);
    for step in 1..=12 {
        fs::write(root.join("notes.txt"), notes(200 + step * 3)).unwrap();
        fs::write(root.join("src/app.txt"), format!("version {step}\n")).unwrap();
        commit(root, START + step as i64 * 10, &format!("step {step}"));
    }
    fs::write(root.join("config.env"), format!("TOKEN={}\n", sample_hosting_token())).unwrap();
    commit(root, START + 200, "add the configuration");
    fs::write(root.join("config.env"), "TOKEN=\n").unwrap();
    commit(root, START + 210, "remove the token");
    run(root, START + 220, &["tag", "-a", "v1", "-m", "version one"]);
    run(root, START + 230, &["checkout", "-q", "side"]);
    fs::write(root.join("side.txt"), "work on the side\n").unwrap();
    commit(root, START + 240, "side work");
    run(root, START + 250, &["checkout", "-q", "main"]);
    run(root, START + 260, &["merge", "-q", "--no-ff", "side", "-m", "merge the side branch"]);
}

fn compare(root: &Path, shape: &str) {
    let mut repository = Repository::open(root).unwrap();
    let mut objects = 0;
    for line in run(root, START, &["rev-list", "--all", "--objects"]).lines() {
        let hex = line.split_whitespace().next().unwrap();
        let kind = run(root, START, &["cat-file", "-t", hex]);
        let expected = raw(root, START, &["cat-file", kind.trim(), hex]);
        match repository.read(&ObjectId::from_hex(hex).unwrap(), git::MAX_OBJECT_BYTES).unwrap() {
            Fetched::Found(found, data) => {
                assert_eq!(Some(found), ObjectKind::parse(kind.trim()), "{shape}: object {hex}");
                assert!(data == expected, "{shape}: object {hex} differs from what git prints");
            }
            other => panic!("{shape}: object {hex} gave {other:?}"),
        }
        objects += 1;
    }
    assert!(objects > 50, "{shape}: only {objects} objects were compared");

    let commits: BTreeSet<String> = run(root, START, &["rev-list", "--all"]).lines().map(str::to_string).collect();
    let mut expected_files: BTreeSet<(String, String)> = BTreeSet::new();
    for commit in &commits {
        for line in run(root, START, &["ls-tree", "-r", commit]).lines() {
            let (details, path) = line.split_once('\t').unwrap();
            let id = details.split_whitespace().nth(2).unwrap();
            if !path.ends_with(".bin") {
                expected_files.insert((path.to_string(), id.to_string()));
            }
        }
    }
    let mut seen_commits = BTreeSet::new();
    let mut visits = 0;
    let mut token_commit = String::new();
    let stats = git::walk_history(
        &mut repository,
        &HistoryOptions::default(),
        &AtomicBool::new(false),
        &mut |commit, path, content| {
            seen_commits.insert(commit.id.to_hex());
            visits += 1;
            if path == "config.env" && content.contains(&sample_hosting_token()) {
                token_commit = commit.subject.clone();
            }
        },
    );
    assert_eq!(stats.commits_scanned as usize, commits.len(), "{shape}");
    assert!(seen_commits.is_subset(&commits), "{shape}");
    assert_eq!(visits, expected_files.len(), "{shape}");
    assert_eq!(stats.files_scanned as usize, expected_files.len(), "{shape}");
    assert_eq!((stats.binary, stats.unreadable, stats.missing, stats.too_large), (1, 0, 0, 0), "{shape}");
    assert!(stats.complete(), "{shape}");
    assert_eq!(token_commit, "add the configuration", "{shape}");

    let names: BTreeSet<String> = repository.references().into_iter().map(|reference| reference.name).collect();
    for name in ["HEAD", "refs/heads/main", "refs/heads/side", "refs/tags/v1"] {
        assert!(names.contains(name), "{shape}: {name} is missing from {names:?}");
    }
}

#[test]
fn a_repository_written_by_git_is_read_the_same_way_git_reads_it() {
    if !git_is_installed() {
        assert!(std::env::var_os("CI").is_none(), "git is needed for this test in continuous integration");
        return;
    }
    let directory = TempDir::new("git-real");
    let root = directory.path();
    build(root);
    compare(root, "loose objects");
    assert_eq!(Repository::open(root).unwrap().pack_count(), 0);

    run(root, START, &["repack", "-q", "-a", "-d", "-f", "--depth=20", "--window=20"]);
    run(root, START, &["pack-refs", "--all"]);
    let packed = Repository::open(root).unwrap();
    assert_eq!(packed.pack_count(), 1);
    assert!(packed.packed_object_count() > 50);
    compare(root, "one pack with offset deltas");

    run(
        root,
        START,
        &["-c", "repack.useDeltaBaseOffset=false", "repack", "-q", "-a", "-d", "-f", "--depth=20", "--window=20"],
    );
    compare(root, "one pack with reference deltas");

    fs::write(root.join("notes.txt"), notes(300)).unwrap();
    commit(root, START + 300, "after packing");
    run(root, START + 310, &["repack", "-q", "-d"]);
    fs::write(root.join("notes.txt"), notes(310)).unwrap();
    commit(root, START + 320, "still loose");
    assert_eq!(Repository::open(root).unwrap().pack_count(), 2);
    compare(root, "two packs and loose objects");
}
