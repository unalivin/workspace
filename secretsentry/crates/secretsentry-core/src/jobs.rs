use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::Duration;

use crate::error::{AppError, Result};
use crate::git::HistoryOptions;
use crate::scans::{Scan, Scans, Status};
use crate::service::{App, Options};
use crate::settings;
use crate::tree;
use crate::words;

const STOPPED: &str = "the scan stopped unexpectedly: start it again";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Files,
    History,
    All,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Files, Source::History, Source::All];

    pub fn parse(text: &str) -> Option<Source> {
        let wanted = text.trim().to_lowercase();
        Source::ALL.into_iter().find(|source| source.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Source::Files => "files",
            Source::History => "history",
            Source::All => "all",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Source::Files => "Files",
            Source::History => "Git history",
            Source::All => "Files and Git history",
        }
    }

    pub fn files(self) -> bool {
        self != Source::History
    }

    pub fn history(self) -> bool {
        self != Source::Files
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub root: PathBuf,
    pub source: Source,
    pub max_file_bytes: u64,
    pub max_commits: u64,
}

impl Request {
    pub fn from_settings(app: &App, root: &str, source: Source) -> Result<Request> {
        let root = root.trim();
        if root.is_empty() {
            return Err(AppError::invalid(
                "path",
                "no directory was given",
                "Name the directory that contains the files to scan",
            ));
        }
        let kib = app.settings().number(settings::KEY_SCAN_MAX_FILE_KIB)?;
        let commits = app.settings().number(settings::KEY_SCAN_MAX_COMMITS)?;
        Ok(Request {
            root: PathBuf::from(root),
            source,
            max_file_bytes: u64::try_from(kib).unwrap_or(1024).saturating_mul(1024),
            max_commits: u64::try_from(commits).unwrap_or(u64::MAX),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub scans: Vec<Scan>,
    pub portable: BTreeSet<String>,
}

impl Report {
    pub fn cancelled(&self) -> bool {
        self.scans.iter().any(|scan| scan.status == Status::Cancelled)
    }

    pub fn summary(&self) -> String {
        let Some(first) = self.scans.first() else {
            return "Nothing was scanned".to_string();
        };
        let scope: Vec<String> = self.scans.iter().map(Scan::scope_label).collect();
        let new: i64 = self.scans.iter().map(|scan| scan.findings_new).sum();
        let lead = if self.cancelled() { "Cancelled the scan of" } else { "Scanned" };
        format!(
            "{lead} {}: {}, {}",
            first.root,
            scope.join(", "),
            words::count(new, "new likely secret", "new likely secrets")
        )
    }
}

pub fn run(scans: &Scans<'_>, request: &Request, cancel: &AtomicBool) -> Result<Report> {
    let mut report = Report::default();
    if request.source.files() {
        let options = tree::Options { max_file_bytes: request.max_file_bytes, ..tree::Options::default() };
        let outcome = scans.scan_tree(&request.root, &options, cancel)?;
        report.portable.extend(outcome.portable);
        report.scans.push(outcome.scan);
    }
    if request.source.history() && !cancel.load(Ordering::Relaxed) {
        let options = HistoryOptions { max_blob_bytes: request.max_file_bytes, max_commits: request.max_commits };
        let outcome = scans.scan_history(&request.root, &options, cancel)?;
        report.portable.extend(outcome.portable);
        report.scans.push(outcome.scan);
    }
    Ok(report)
}

pub struct Job {
    root: String,
    source: Source,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Result<Report>>,
}

impl Job {
    pub fn start(options: Options, request: Request) -> Job {
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let root = request.root.display().to_string();
        let source = request.source;
        let flag = Arc::clone(&cancel);
        thread::spawn(move || {
            let outcome = App::open(options).and_then(|app| run(&app.scans(), &request, &flag));
            let _ = sender.send(outcome);
        });
        Job { root, source, cancel, receiver }
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn source(&self) -> Source {
        self.source
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelling(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn poll(&self) -> Option<Result<Report>> {
        match self.receiver.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(AppError::Io(STOPPED.to_string()))),
        }
    }

    pub fn wait(&self, limit: Duration) -> Option<Result<Report>> {
        match self.receiver.recv_timeout(limit) {
            Ok(outcome) => Some(outcome),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => Some(Err(AppError::Io(STOPPED.to_string()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::*;
    use crate::clock::{Clock, Zone};
    use crate::platform::{Env, Os};
    use crate::testkit::{GitFixture, TempDir, fixed_time, sample_cloud_key, sample_hosting_token};

    fn options(home: &TempDir) -> Options {
        let directory = home.path().to_string_lossy().into_owned();
        Options {
            os: Os::Linux,
            env: Env::from_pairs(&[("HOME", directory.as_str())]),
            config_dir: None,
            database_path: None,
            overrides: BTreeMap::new(),
            clock: Clock::fixed(fixed_time()),
            zone: Zone::Utc,
            read_only: false,
        }
    }

    fn repository(home: &TempDir) -> String {
        let root = home.path().join("project");
        fs::create_dir_all(&root).unwrap();
        let git = GitFixture::create(&root);
        let secret = git.blob(&format!("TOKEN={}\n", sample_hosting_token()));
        let cleaned = git.blob("TOKEN=\n");
        let first = git.commit(&git.tree(&[("100644", "config.env", &secret)]), &[], 1_700_000_000, "add settings");
        let second =
            git.commit(&git.tree(&[("100644", "config.env", &cleaned)]), &[&first], 1_700_000_100, "remove the token");
        git.set_reference("refs/heads/main", &second);
        fs::write(root.join("config.env"), "TOKEN=\n").unwrap();
        fs::write(root.join("deploy.sh"), format!("export AWS_KEY=\"{}\"\n", sample_cloud_key())).unwrap();
        root.to_string_lossy().into_owned()
    }

    #[test]
    fn a_request_reads_its_limits_from_the_settings_and_needs_a_directory() {
        let home = TempDir::new("jobs-request");
        let app = App::open(options(&home)).unwrap();
        let request = Request::from_settings(&app, " /srv/project ", Source::All).unwrap();
        assert_eq!(request.root, PathBuf::from("/srv/project"));
        assert_eq!((request.max_file_bytes, request.max_commits), (1024 * 1024, 200_000));
        app.settings().set(settings::KEY_SCAN_MAX_FILE_KIB, "4").unwrap();
        assert_eq!(Request::from_settings(&app, "x", Source::Files).unwrap().max_file_bytes, 4096);
        assert!(Request::from_settings(&app, "  ", Source::Files).unwrap_err().is_usage());
        assert_eq!(Source::parse(" History "), Some(Source::History));
        assert_eq!(Source::parse("everything"), None);
        assert!(Source::All.files() && Source::All.history() && !Source::Files.history() && !Source::History.files());
    }

    #[test]
    fn a_run_scans_the_chosen_sources_and_sums_up_what_it_found() {
        let home = TempDir::new("jobs-run");
        let app = App::open(options(&home)).unwrap();
        let root = repository(&home);
        let cancel = AtomicBool::new(false);
        let files = run(&app.scans(), &Request::from_settings(&app, &root, Source::Files).unwrap(), &cancel).unwrap();
        assert_eq!(files.scans.len(), 1);
        assert!(files.summary().starts_with("Scanned ") && files.summary().ends_with("1 new likely secret"));
        let both = run(&app.scans(), &Request::from_settings(&app, &root, Source::All).unwrap(), &cancel).unwrap();
        assert_eq!(both.scans.iter().map(|scan| scan.kind.as_str()).collect::<Vec<_>>(), ["tree", "history"]);
        assert!(both.summary().contains("2 commits") && both.summary().ends_with("1 new likely secret"));
        assert_eq!(app.findings().count().unwrap(), 2);
        assert_eq!(Report::default().summary(), "Nothing was scanned");
        let missing = Request::from_settings(&app, &home.path().join("none").to_string_lossy(), Source::Files).unwrap();
        assert!(run(&app.scans(), &missing, &cancel).is_err());
    }

    #[test]
    fn a_cancelled_run_stops_before_the_history_and_says_so() {
        let home = TempDir::new("jobs-cancel");
        let app = App::open(options(&home)).unwrap();
        let root = repository(&home);
        let cancel = AtomicBool::new(true);
        let report = run(&app.scans(), &Request::from_settings(&app, &root, Source::All).unwrap(), &cancel).unwrap();
        assert_eq!(report.scans.len(), 1);
        assert!(report.cancelled());
        assert!(report.summary().starts_with("Cancelled the scan of "), "{}", report.summary());
        assert_eq!(app.findings().count().unwrap(), 0);
    }

    #[test]
    fn a_job_scans_in_the_background_with_its_own_connection() {
        let home = TempDir::new("jobs-thread");
        let app = App::open(options(&home)).unwrap();
        let root = repository(&home);
        let job = Job::start(options(&home), Request::from_settings(&app, &root, Source::All).unwrap());
        assert_eq!((job.root(), job.source(), job.cancelling()), (root.as_str(), Source::All, false));
        let report = job.wait(Duration::from_secs(60)).expect("the scan finishes").unwrap();
        assert_eq!(report.scans.len(), 2);
        assert_eq!(app.findings().count().unwrap(), 2);
        assert_eq!(app.scans().count().unwrap(), 2);
        assert!(job.poll().is_some_and(|outcome| outcome.is_err()));

        let failing =
            Job::start(options(&home), Request::from_settings(&app, "/no/such/place", Source::Files).unwrap());
        failing.cancel();
        assert!(failing.cancelling());
        assert!(failing.wait(Duration::from_secs(60)).expect("the scan reports").is_err());
    }
}
