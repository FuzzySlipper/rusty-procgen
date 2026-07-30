#[allow(unused_imports)]
use crate::*;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

static PAIR_WRITE_NONCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to encode JSON for {}: {error}", path.display()))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(crate) fn write_json_pair_atomic<T: Serialize, U: Serialize>(
    first_path: &Path,
    first_value: &T,
    second_path: &Path,
    second_value: &U,
) -> Result<(), String> {
    write_json_pair_atomic_with_hook(first_path, first_value, second_path, second_value, |_| {
        Ok(())
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PairWriteStep {
    FirstStage,
    SecondStage,
    FirstBackup,
    SecondBackup,
    FirstCommit,
    SecondCommit,
}

fn write_json_pair_atomic_with_hook<T, U, F>(
    first_path: &Path,
    first_value: &T,
    second_path: &Path,
    second_value: &U,
    mut before: F,
) -> Result<(), String>
where
    T: Serialize,
    U: Serialize,
    F: FnMut(PairWriteStep) -> Result<(), String>,
{
    let first_bytes = encode_pretty_json(first_path, first_value)?;
    let second_bytes = encode_pretty_json(second_path, second_value)?;
    let first = PairWriteTarget::prepare(first_path)?;
    let second = PairWriteTarget::prepare(second_path)?;
    validate_distinct_pair_targets(&first, &second)?;

    let mut state = PairWriteState::new(first, second);
    let publication = (|| {
        before(PairWriteStep::FirstStage)?;
        state.first.stage(&first_bytes)?;
        before(PairWriteStep::SecondStage)?;
        state.second.stage(&second_bytes)?;
        before(PairWriteStep::FirstBackup)?;
        state.first.backup()?;
        before(PairWriteStep::SecondBackup)?;
        state.second.backup()?;
        before(PairWriteStep::FirstCommit)?;
        state.first.commit()?;
        before(PairWriteStep::SecondCommit)?;
        state.second.commit()?;
        Ok(())
    })();

    if let Err(error) = publication {
        return match state.rollback() {
            Ok(()) => Err(error),
            Err(rollback) => Err(format!("{error}; rollback failed: {rollback}")),
        };
    }
    state.finish()
}

fn encode_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<Vec<u8>, String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to encode JSON for {}: {error}", path.display()))?;
    Ok(format!("{text}\n").into_bytes())
}

struct PairWriteTarget {
    destination: PathBuf,
    resolved_destination: PathBuf,
    stage_path: PathBuf,
    backup_path: PathBuf,
    had_original: bool,
    staged: bool,
    backed_up: bool,
    committed: bool,
}

impl PairWriteTarget {
    fn prepare(destination: &Path) -> Result<Self, String> {
        let file_name = destination.file_name().ok_or_else(|| {
            format!(
                "paired JSON output {} must name a file",
                destination.display()
            )
        })?;
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        let resolved_parent = fs::canonicalize(parent)
            .map_err(|error| format!("failed to resolve {}: {error}", parent.display()))?;
        let resolved_destination = resolved_parent.join(file_name);
        let had_original = match fs::symlink_metadata(destination) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "paired JSON output {} must not be a symbolic link",
                        destination.display()
                    ));
                }
                if !metadata.is_file() {
                    return Err(format!(
                        "paired JSON output {} must be a file target",
                        destination.display()
                    ));
                }
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(format!(
                    "failed to inspect paired JSON output {}: {error}",
                    destination.display()
                ));
            }
        };
        let nonce = PAIR_WRITE_NONCE.fetch_add(1, AtomicOrdering::Relaxed);
        let stem = file_name.to_string_lossy();
        let unique = format!("{}-{nonce}", std::process::id());
        let stage_path = resolved_parent.join(format!(".{stem}.{unique}.stage"));
        let backup_path = resolved_parent.join(format!(".{stem}.{unique}.backup"));
        if stage_path.exists() || backup_path.exists() {
            return Err(format!(
                "paired JSON temporary path collision beside {}",
                destination.display()
            ));
        }
        Ok(Self {
            destination: destination.to_path_buf(),
            resolved_destination,
            stage_path,
            backup_path,
            had_original,
            staged: false,
            backed_up: false,
            committed: false,
        })
    }

    fn stage(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&self.stage_path)
            .map_err(|error| {
                format!(
                    "failed to stage paired JSON output {}: {error}",
                    self.destination.display()
                )
            })?;
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&self.stage_path);
            return Err(format!(
                "failed to stage paired JSON output {}: {error}",
                self.destination.display()
            ));
        }
        self.staged = true;
        Ok(())
    }

    fn backup(&mut self) -> Result<(), String> {
        if self.had_original {
            fs::rename(&self.destination, &self.backup_path).map_err(|error| {
                format!(
                    "failed to prepare paired JSON output {}: {error}",
                    self.destination.display()
                )
            })?;
            self.backed_up = true;
        }
        Ok(())
    }

    fn commit(&mut self) -> Result<(), String> {
        fs::rename(&self.stage_path, &self.destination).map_err(|error| {
            format!(
                "failed to commit paired JSON output {}: {error}",
                self.destination.display()
            )
        })?;
        self.staged = false;
        self.committed = true;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        if self.committed {
            if let Err(error) = fs::remove_file(&self.destination) {
                errors.push(format!(
                    "failed to remove replacement {}: {error}",
                    self.destination.display()
                ));
            }
            self.committed = false;
        }
        if self.backed_up {
            if let Err(error) = fs::rename(&self.backup_path, &self.destination) {
                errors.push(format!(
                    "failed to restore {}: {error}",
                    self.destination.display()
                ));
            } else {
                self.backed_up = false;
            }
        }
        if self.staged {
            if let Err(error) = fs::remove_file(&self.stage_path) {
                errors.push(format!(
                    "failed to remove staged output {}: {error}",
                    self.stage_path.display()
                ));
            } else {
                self.staged = false;
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    fn finish(&mut self) -> Result<(), String> {
        if self.backed_up {
            fs::remove_file(&self.backup_path).map_err(|error| {
                format!(
                    "paired JSON outputs committed but failed to remove backup {}: {error}",
                    self.backup_path.display()
                )
            })?;
            self.backed_up = false;
        }
        Ok(())
    }
}

struct PairWriteState {
    first: PairWriteTarget,
    second: PairWriteTarget,
}

impl PairWriteState {
    fn new(first: PairWriteTarget, second: PairWriteTarget) -> Self {
        Self { first, second }
    }

    fn rollback(&mut self) -> Result<(), String> {
        let second = self.second.rollback();
        let first = self.first.rollback();
        match (first, second) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(first), Ok(())) => Err(first),
            (Ok(()), Err(second)) => Err(second),
            (Err(first), Err(second)) => Err(format!("{first}; {second}")),
        }
    }

    fn finish(&mut self) -> Result<(), String> {
        self.first.finish()?;
        self.second.finish()
    }
}

fn validate_distinct_pair_targets(
    first: &PairWriteTarget,
    second: &PairWriteTarget,
) -> Result<(), String> {
    if existing_files_alias(&first.destination, &second.destination)? {
        return Err(format!(
            "paired JSON outputs must be distinct: {} and {}",
            first.destination.display(),
            second.destination.display()
        ));
    }
    let paths = [
        ("first output", &first.resolved_destination),
        ("first stage", &first.stage_path),
        ("first backup", &first.backup_path),
        ("second output", &second.resolved_destination),
        ("second stage", &second.stage_path),
        ("second backup", &second.backup_path),
    ];
    for left in 0..paths.len() {
        for right in (left + 1)..paths.len() {
            if paths[left].1 == paths[right].1 {
                return Err(format!(
                    "paired JSON output and temporary paths must be pairwise distinct: {} {} conflicts with {} {}",
                    paths[left].0,
                    paths[left].1.display(),
                    paths[right].0,
                    paths[right].1.display()
                ));
            }
        }
    }
    Ok(())
}

fn existing_files_alias(first: &Path, second: &Path) -> Result<bool, String> {
    let (first_metadata, second_metadata) = match (fs::metadata(first), fs::metadata(second)) {
        (Ok(first), Ok(second)) => (first, second),
        (Err(first_error), _) if first_error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        (_, Err(second_error)) if second_error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        (Err(error), _) => {
            return Err(format!("failed to inspect {}: {error}", first.display()));
        }
        (_, Err(error)) => {
            return Err(format!("failed to inspect {}: {error}", second.display()));
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(first_metadata.dev() == second_metadata.dev()
            && first_metadata.ino() == second_metadata.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = (first_metadata, second_metadata);
        Ok(false)
    }
}

pub(crate) fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(crate) fn append_transcript(
    path: Option<&Path>,
    command: &str,
    state: Option<&Path>,
    receipt: Option<&Path>,
    seed: Option<u64>,
    args: JsonValue,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let event = json!({
        "kind": "tool_event",
        "command": command,
        "state": state.map(display_path),
        "receipt": receipt.map(display_path),
        "seed": seed,
        "args": args
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    writeln!(file, "{event}")
        .map_err(|error| format!("failed to write transcript {}: {error}", path.display()))
}

pub(crate) fn receipt(
    command: &str,
    seed: Option<u64>,
    input_hash: Option<&str>,
    output_hash: Option<&str>,
    output_ref: Option<&Path>,
    diagnostics: Vec<Diagnostic>,
) -> Receipt {
    Receipt {
        kind: "rusty_procgen.receipt.v1".to_owned(),
        schema_version: 1,
        command: command.to_owned(),
        status: "ok".to_owned(),
        seed,
        input_hash: input_hash.map(str::to_owned),
        output_hash: output_hash.map(str::to_owned),
        output_ref: output_ref.map(display_path),
        diagnostics,
    }
}

pub(crate) fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(format!("fnv1a64:{:016x}", fnv1a64(&bytes)))
}

pub(crate) fn hash_json<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("failed to encode hash input: {error}"))?;
    Ok(format!("fnv1a64:{:016x}", fnv1a64(&bytes)))
}

pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn stable_suffix(seed: u64) -> String {
    format!("{:04x}", seed & 0xffff)
}

pub(crate) fn slugify_label(label: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for character in label.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('_');
            last_was_separator = true;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    if slug.is_empty() {
        "fork".to_owned()
    } else {
        slug
    }
}

pub(crate) fn fatal(
    code: &str,
    node: Option<&str>,
    edge: Option<&str>,
    detail: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        node: node.map(str::to_owned),
        edge: edge.map(str::to_owned),
        detail: detail.into(),
        repair_hint: None,
    }
}

pub(crate) fn fatal_with_hint(
    code: &str,
    node: Option<&str>,
    edge: Option<&str>,
    detail: impl Into<String>,
    repair_hint: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        node: node.map(str::to_owned),
        edge: edge.map(str::to_owned),
        detail: detail.into(),
        repair_hint: Some(repair_hint.into()),
    }
}

pub(crate) fn warning_with_hint(
    code: &str,
    node: Option<&str>,
    edge: Option<&str>,
    detail: impl Into<String>,
    repair_hint: impl Into<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Warning,
        node: node.map(str::to_owned),
        edge: edge.map(str::to_owned),
        detail: detail.into(),
        repair_hint: Some(repair_hint.into()),
    }
}

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod pair_write_tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static PAIR_WRITE_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn paired_json_publication_rolls_back_every_stage_and_commit_failure() {
        let _guard = PAIR_WRITE_TEST_LOCK.lock().expect("lock pair-write tests");
        for fail_at in [
            PairWriteStep::FirstStage,
            PairWriteStep::SecondStage,
            PairWriteStep::FirstBackup,
            PairWriteStep::SecondBackup,
            PairWriteStep::FirstCommit,
            PairWriteStep::SecondCommit,
        ] {
            let root = unique_test_dir("rollback");
            fs::create_dir_all(&root).expect("create rollback fixture");
            let first = root.join("result.json");
            let second = root.join("trace.json");
            fs::write(&first, b"original-result\n").expect("write result sentinel");
            fs::write(&second, b"original-trace\n").expect("write trace sentinel");
            let error = write_json_pair_atomic_with_hook(
                &first,
                &json!({"replacement": "result"}),
                &second,
                &json!({"replacement": "trace"}),
                |step| {
                    if step == fail_at {
                        Err(format!("injected {step:?} failure"))
                    } else {
                        Ok(())
                    }
                },
            )
            .expect_err("injected publication must fail");
            assert!(error.contains("injected"));
            assert_eq!(
                fs::read(&first).expect("read result sentinel"),
                b"original-result\n"
            );
            assert_eq!(
                fs::read(&second).expect("read trace sentinel"),
                b"original-trace\n"
            );
            let names = fs::read_dir(&root)
                .expect("read fixture directory")
                .map(|entry| {
                    entry
                        .expect("directory entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                names,
                BTreeSet::from(["result.json".to_owned(), "trace.json".to_owned()])
            );
            fs::remove_dir_all(root).expect("remove rollback fixture");
        }
    }

    #[test]
    fn paired_json_publication_rejects_aliases_before_mutation() {
        let _guard = PAIR_WRITE_TEST_LOCK.lock().expect("lock pair-write tests");
        let root = unique_test_dir("aliases");
        fs::create_dir_all(&root).expect("create alias fixture");
        let first = root.join("result.json");
        fs::write(&first, b"original\n").expect("write alias sentinel");
        let exact = write_json_pair_atomic(
            &first,
            &json!({"replacement": 1}),
            &first,
            &json!({"replacement": 2}),
        )
        .expect_err("exact alias must reject");
        assert!(exact.contains("must be distinct"));
        assert_eq!(
            fs::read(&first).expect("read alias sentinel"),
            b"original\n"
        );

        #[cfg(unix)]
        {
            let hard_link = root.join("trace-hard-link.json");
            fs::hard_link(&first, &hard_link).expect("create hard link");
            let alias = write_json_pair_atomic(
                &first,
                &json!({"replacement": 1}),
                &hard_link,
                &json!({"replacement": 2}),
            )
            .expect_err("hard-link alias must reject");
            assert!(alias.contains("must be distinct"));
            assert_eq!(
                fs::read(&first).expect("read hard-link sentinel"),
                b"original\n"
            );
            assert_eq!(
                fs::read(&hard_link).expect("read hard-link alias"),
                b"original\n"
            );
        }
        fs::remove_dir_all(root).expect("remove alias fixture");
    }

    #[test]
    fn paired_json_publication_keeps_destinations_out_of_temporary_namespace() {
        let _guard = PAIR_WRITE_TEST_LOCK.lock().expect("lock pair-write tests");
        for collision in [
            TemporaryCollision::SecondAtFirstStage,
            TemporaryCollision::SecondAtFirstBackup,
            TemporaryCollision::FirstAtSecondStage,
            TemporaryCollision::FirstAtSecondBackup,
        ] {
            let root = unique_test_dir("temporary-namespace");
            fs::create_dir_all(&root).expect("create temporary namespace fixture");
            let next_nonce = PAIR_WRITE_NONCE.load(AtomicOrdering::Relaxed);
            let process = std::process::id();
            let (first, second, sentinel) = match collision {
                TemporaryCollision::SecondAtFirstStage => {
                    let first = root.join("result.json");
                    let second = root.join(format!(".result.json.{process}-{next_nonce}.stage"));
                    fs::write(&first, b"original-result\n").expect("write first sentinel");
                    (first.clone(), second, first)
                }
                TemporaryCollision::SecondAtFirstBackup => {
                    let first = root.join("result.json");
                    let second = root.join(format!(".result.json.{process}-{next_nonce}.backup"));
                    fs::write(&first, b"original-result\n").expect("write first sentinel");
                    (first.clone(), second, first)
                }
                TemporaryCollision::FirstAtSecondStage => {
                    let second = root.join("trace.json");
                    let first =
                        root.join(format!(".trace.json.{process}-{}.stage", next_nonce + 1));
                    fs::write(&second, b"original-trace\n").expect("write second sentinel");
                    (first, second.clone(), second)
                }
                TemporaryCollision::FirstAtSecondBackup => {
                    let second = root.join("trace.json");
                    let first =
                        root.join(format!(".trace.json.{process}-{}.backup", next_nonce + 1));
                    fs::write(&second, b"original-trace\n").expect("write second sentinel");
                    (first, second.clone(), second)
                }
            };

            let error = write_json_pair_atomic(
                &first,
                &json!({"replacement": "result"}),
                &second,
                &json!({"replacement": "trace"}),
            )
            .expect_err("destination in the pair temporary namespace must reject");
            assert!(error.contains("must be pairwise distinct"), "{error}");
            assert_eq!(
                fs::read(&sentinel).expect("read unchanged sentinel"),
                if sentinel == first {
                    b"original-result\n".as_slice()
                } else {
                    b"original-trace\n".as_slice()
                }
            );
            assert!(
                !if sentinel == first {
                    second.exists()
                } else {
                    first.exists()
                },
                "colliding destination must not be published"
            );
            let names = fs::read_dir(&root)
                .expect("read temporary namespace fixture")
                .map(|entry| {
                    entry
                        .expect("directory entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                names,
                BTreeSet::from([sentinel
                    .file_name()
                    .expect("sentinel file name")
                    .to_string_lossy()
                    .into_owned()])
            );
            fs::remove_dir_all(root).expect("remove temporary namespace fixture");
        }
    }

    #[derive(Clone, Copy)]
    enum TemporaryCollision {
        SecondAtFirstStage,
        SecondAtFirstBackup,
        FirstAtSecondStage,
        FirstAtSecondBackup,
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rusty-procgen-pair-write-{label}-{}-{unique}",
            std::process::id()
        ))
    }
}
