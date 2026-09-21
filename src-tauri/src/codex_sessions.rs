use super::{
    command_activity_event, evaluate_command, file_activity_event, ActivityEvent, FileEventInput,
    GuardMode,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

const INITIAL_SESSIONS: usize = 8;
const INITIAL_TAIL_BYTES: u64 = 1024 * 1024;
const READ_BYTES_PER_TICK: u64 = 1024 * 1024;
const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;
const SEEN_LIMIT: usize = 4096;
const ACTIVITY_LIMIT: usize = 500;

fn stable_hash(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

struct SessionParser {
    scope: u64,
    source: String,
    paginated: bool,
    seen: HashSet<String>,
    seen_order: VecDeque<String>,
}

impl SessionParser {
    fn new(path: &Path) -> Self {
        Self {
            scope: stable_hash(path),
            source: "Codex".into(),
            paginated: false,
            seen: HashSet::new(),
            seen_order: VecDeque::new(),
        }
    }

    fn read_metadata(&mut self, payload: &Value) {
        let originator = payload["originator"].as_str().unwrap_or_default();
        let source = payload["source"].as_str().unwrap_or_default();
        let label = if originator.to_ascii_lowercase().contains("desktop") {
            "Codex Desktop"
        } else if payload["source"].get("subagent").is_some() {
            "Codex Subagent"
        } else if matches!(source, "cli" | "exec") || originator == "codex-tui" {
            "Codex CLI"
        } else if source == "vscode" {
            "Codex IDE"
        } else {
            "Codex"
        };
        self.source = match payload["cli_version"].as_str() {
            Some(version)
                if version.len() <= 64
                    && version
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c)) =>
            {
                format!("{label} {version}")
            }
            _ => label.into(),
        };
        self.paginated = payload["history_mode"] == "paginated";
    }

    fn parse_line(&mut self, line: &[u8], mode: GuardMode) -> Vec<ActivityEvent> {
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            return vec![];
        };
        let payload = &record["payload"];
        if record["type"] == "session_meta" {
            self.read_metadata(payload);
            return vec![];
        }

        let mut events = match record["type"].as_str() {
            Some("response_item") if !self.paginated => response_events(payload, mode),
            Some("event_msg") => match payload["type"].as_str() {
                Some("item_completed") => {
                    let item = &payload["item"];
                    match item["type"].as_str() {
                        Some("CommandExecution" | "command_execution" | "commandExecution") => {
                            command_event(item, mode).into_iter().collect()
                        }
                        Some("FileChange" | "file_change" | "fileChange") => {
                            file_events(item, mode)
                        }
                        _ => vec![],
                    }
                }
                Some("exec_command_end") => command_event(payload, mode).into_iter().collect(),
                Some("patch_apply_end") => file_events(payload, mode),
                _ => vec![],
            },
            _ => vec![],
        };

        let timestamp = record["timestamp"]
            .as_str()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| {
                value
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::Millis, true)
            });
        events.retain_mut(|event| {
            if event.id.starts_with("record-") {
                event.id = format!(
                    "{:016x}-{}",
                    stable_hash(record["timestamp"].to_string()),
                    event.id
                );
            }
            event.id = format!("codex-{:016x}-{}", self.scope, event.id);
            if !self.seen.insert(event.id.clone()) {
                return false;
            }
            self.seen_order.push_back(event.id.clone());
            if self.seen_order.len() > SEEN_LIMIT {
                if let Some(old) = self.seen_order.pop_front() {
                    self.seen.remove(&old);
                }
            }
            if let Some(timestamp) = timestamp.as_ref() {
                event.timestamp = timestamp.clone();
            }
            event.source = Some(self.source.clone());
            true
        });
        events
    }
}

fn record_id(payload: &Value) -> String {
    payload["call_id"]
        .as_str()
        .or_else(|| payload["id"].as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("record-{:016x}", stable_hash(payload.to_string())))
}

fn command_text(value: &Value) -> Option<String> {
    let command = match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()?
            .join(" "),
        _ => return None,
    };
    (!command.trim().is_empty()).then_some(command)
}

fn command_event(payload: &Value, mode: GuardMode) -> Option<ActivityEvent> {
    let mut command = command_text(payload.get("cmd").or_else(|| payload.get("command"))?)?;
    if let Some(input) = payload["interaction_input"]
        .as_str()
        .filter(|input| !input.trim().is_empty())
    {
        command.push('\n');
        command.push_str(input);
    }
    let mut event = command_activity_event(&command, mode);
    event.id = format!("{}:command", record_id(payload));
    match payload["status"].as_str() {
        Some("declined") => event.title = "命令被拒绝".into(),
        Some("failed") => event.title = "命令执行失败".into(),
        _ => {}
    }
    Some(event)
}

fn response_events(payload: &Value, mode: GuardMode) -> Vec<ActivityEvent> {
    match payload["type"].as_str() {
        Some("function_call") => {
            let arguments = match &payload["arguments"] {
                Value::String(raw) => serde_json::from_str(raw).unwrap_or(Value::Null),
                value => value.clone(),
            };
            tool_events(
                payload["name"].as_str().unwrap_or_default(),
                &arguments,
                &record_id(payload),
                mode,
                0,
            )
        }
        Some("local_shell_call") if payload["action"]["type"] == "exec" => {
            let mut action = payload["action"].clone();
            action["call_id"] = Value::String(record_id(payload));
            command_event(&action, mode).into_iter().collect()
        }
        // Code Mode source is JavaScript, not a shell command. Only inspect the
        // host's recorded calls; paginated sessions use CommandExecution instead.
        Some("custom_tool_call_output" | "function_call_output") => payload
            ["internal_chat_message_metadata_passthrough"]["executed_tool_calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .enumerate()
                    .flat_map(|(index, call)| {
                        tool_events(
                            call["name"].as_str().unwrap_or_default(),
                            &call["arguments"],
                            &format!("{}:{index}", record_id(payload)),
                            mode,
                            0,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => vec![],
    }
}

fn tool_events(
    name: &str,
    arguments: &Value,
    id: &str,
    mode: GuardMode,
    depth: usize,
) -> Vec<ActivityEvent> {
    if depth > 8 {
        return vec![];
    }
    match name {
        "shell"
        | "shell_command"
        | "exec_command"
        | "functions.shell"
        | "functions.shell_command"
        | "functions.exec_command"
        | "tools.shell"
        | "tools.shell_command"
        | "tools.exec_command" => {
            if !arguments.is_object() {
                return vec![];
            }
            let mut payload = arguments.clone();
            payload["call_id"] = Value::String(id.into());
            command_event(&payload, mode).into_iter().collect()
        }
        "multi_tool_use.parallel" | "parallel" => arguments["tool_uses"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .enumerate()
                    .flat_map(|(index, call)| {
                        tool_events(
                            call["recipient_name"].as_str().unwrap_or_default(),
                            &call["parameters"],
                            &format!("{id}:{index}"),
                            mode,
                            depth + 1,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => vec![],
    }
}

fn diff_counts(diff: &str) -> (usize, usize) {
    let mut added = 0;
    let mut removed = 0;
    let mut in_hunk = false;
    for line in diff.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk {
            added += usize::from(line.starts_with('+'));
            removed += usize::from(line.starts_with('-'));
        }
    }
    (added, removed)
}

fn file_events(payload: &Value, mode: GuardMode) -> Vec<ActivityEvent> {
    let Some(changes) = payload["changes"].as_object() else {
        return vec![];
    };
    let failed = matches!(payload["status"].as_str(), Some("failed" | "declined"))
        || payload["success"] == false;
    changes
        .iter()
        .filter_map(|(path, change)| {
            let (kind, added, removed) = match change["type"].as_str() {
                Some("add") => (
                    "file-create",
                    change["content"]
                        .as_str()
                        .map(|content| content.lines().count()),
                    Some(0),
                ),
                Some("delete") => (
                    "file-delete",
                    Some(0),
                    change["content"]
                        .as_str()
                        .map(|content| content.lines().count()),
                ),
                Some("update") => {
                    let counts = change["unified_diff"].as_str().map(diff_counts);
                    (
                        "file-modify",
                        counts.map(|counts| counts.0),
                        counts.map(|counts| counts.1),
                    )
                }
                _ => return None,
            };
            let mut paths = vec![path.clone()];
            if let Some(destination) = change["move_path"]
                .as_str()
                .filter(|destination| *destination != path)
            {
                paths.push(destination.into());
            }
            let verb = if kind == "file-delete" {
                "Remove-Item"
            } else {
                "apply_patch"
            };
            let decision = evaluate_command(&format!("{verb} {}", paths.join(" ")), mode);
            let mut event = file_activity_event(FileEventInput {
                kind: if failed { "command" } else { kind }.into(),
                paths: paths
                    .into_iter()
                    .map(|path| super::redact_command(&path))
                    .collect(),
                line_delta: if failed {
                    None
                } else {
                    added
                        .zip(removed)
                        .map(|(added, removed)| added as i64 - removed as i64)
                },
                lines_added: if failed { None } else { added },
                lines_removed: if failed { None } else { removed },
                summary: None,
                source: None,
            });
            event.id = format!("{}:file-{:016x}", record_id(payload), stable_hash(path));
            event.severity = decision.severity;
            if failed {
                event.title = "文件变更未完成".into();
                event.summary = "文件变更失败或被拒绝，未计入已完成的文件活动。".into();
            } else if event.severity != "info" {
                event.summary = decision.message;
            }
            Some(event)
        })
        .collect()
}

struct RolloutFile {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

fn find_rollouts(root: &Path) -> Vec<RolloutFile> {
    let mut files = vec![];
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                    if let Ok(metadata) = entry.metadata() {
                        files.push(RolloutFile {
                            path: entry.path(),
                            len: metadata.len(),
                            modified: metadata.modified().ok(),
                        });
                    }
                }
            }
        }
    }
    files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    files
}

struct RolloutCursor {
    offset: u64,
    pending: Vec<u8>,
    discard_line: bool,
    initialized: bool,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    parser: SessionParser,
}

impl RolloutCursor {
    fn new(path: &Path, offset: u64) -> Self {
        Self {
            offset,
            pending: vec![],
            discard_line: false,
            initialized: false,
            modified: None,
            created: None,
            parser: SessionParser::new(path),
        }
    }

    fn read(&mut self, path: &Path, mode: GuardMode) -> Vec<ActivityEvent> {
        let Ok(mut file) = fs::File::open(path) else {
            return vec![];
        };
        let Ok(metadata) = file.metadata() else {
            return vec![];
        };
        let modified = metadata.modified().ok();
        let created = metadata.created().ok();
        if metadata.len() < self.offset
            || (self.initialized
                && (self.created != created
                    || (metadata.len() == self.offset && self.modified != modified)))
        {
            *self = Self::new(path, 0);
        }
        self.modified = modified;
        self.created = created;
        if metadata.len() == self.offset {
            return vec![];
        }
        if !self.initialized {
            let mut first_line = vec![];
            let _ = BufReader::new((&mut file).take(MAX_LINE_BYTES as u64))
                .read_until(b'\n', &mut first_line);
            if let Ok(value) = serde_json::from_slice::<Value>(&first_line) {
                if value["type"] == "session_meta" {
                    self.parser.read_metadata(&value["payload"]);
                }
            }
            if self.offset > 0 {
                let mut previous = [0];
                if file.seek(SeekFrom::Start(self.offset - 1)).is_err()
                    || file.read_exact(&mut previous).is_err()
                {
                    return vec![];
                }
                self.discard_line = previous[0] != b'\n';
            }
            self.initialized = true;
        }
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return vec![];
        }
        let mut bytes = vec![];
        if file
            .take(READ_BYTES_PER_TICK)
            .read_to_end(&mut bytes)
            .is_err()
        {
            return vec![];
        }
        self.offset += bytes.len() as u64;
        let mut events = vec![];
        for fragment in bytes.split_inclusive(|byte| *byte == b'\n') {
            let complete = fragment.last() == Some(&b'\n');
            if !self.discard_line {
                if self.pending.len() + fragment.len() > MAX_LINE_BYTES {
                    self.pending.clear();
                    self.discard_line = true;
                } else {
                    self.pending.extend_from_slice(fragment);
                }
            }
            if complete {
                if !self.discard_line {
                    events.extend(self.parser.parse_line(&self.pending, mode));
                }
                self.pending.clear();
                self.discard_line = false;
            }
        }
        events
    }
}

struct SessionMonitor {
    root: PathBuf,
    cursors: HashMap<PathBuf, RolloutCursor>,
}

impl SessionMonitor {
    fn new(root: PathBuf) -> Self {
        let cursors = find_rollouts(&root)
            .into_iter()
            .enumerate()
            .map(|(index, file)| {
                let offset = if index < INITIAL_SESSIONS {
                    file.len.saturating_sub(INITIAL_TAIL_BYTES)
                } else {
                    file.len
                };
                let cursor = RolloutCursor::new(&file.path, offset);
                (file.path, cursor)
            })
            .collect();
        Self { root, cursors }
    }

    fn refresh(&mut self) {
        let files = find_rollouts(&self.root);
        let existing: HashSet<_> = files.iter().map(|file| file.path.clone()).collect();
        self.cursors.retain(|path, _| existing.contains(path));
        for file in files {
            self.cursors
                .entry(file.path.clone())
                .or_insert_with(|| RolloutCursor::new(&file.path, 0));
        }
    }

    fn poll(&mut self, mode: GuardMode) -> Vec<ActivityEvent> {
        let mut events: Vec<_> = self
            .cursors
            .iter_mut()
            .flat_map(|(path, cursor)| cursor.read(path, mode))
            .collect();
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));
        events
    }
}

pub(super) fn monitor_codex(
    root: PathBuf,
    activity: Arc<Mutex<Vec<ActivityEvent>>>,
    stop: Arc<AtomicBool>,
    mode: GuardMode,
) {
    let mut monitor = SessionMonitor::new(root);
    let mut tick = 0u32;
    while !stop.load(Ordering::SeqCst) {
        if tick % 5 == 0 {
            monitor.refresh();
        }
        let events = monitor.poll(mode);
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if !events.is_empty() {
            if let Ok(mut activity) = activity.lock() {
                let existing: HashSet<_> = activity.iter().map(|event| event.id.clone()).collect();
                activity.extend(
                    events
                        .into_iter()
                        .filter(|event| !existing.contains(&event.id)),
                );
                activity
                    .sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));
                let excess = activity.len().saturating_sub(ACTIVITY_LIMIT);
                activity.drain(..excess);
            }
        }
        tick = tick.wrapping_add(1);
        thread::sleep(Duration::from_millis(800));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn record(payload: Value) -> Vec<u8> {
        serde_json::to_vec(
            &json!({"timestamp":"2026-09-05T10:00:00Z","type":"response_item","payload":payload}),
        )
        .unwrap()
    }

    fn completed(item: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({"timestamp":"2026-09-05T10:00:01Z","type":"event_msg","payload":{"type":"item_completed","item":item}})).unwrap()
    }

    fn parser() -> SessionParser {
        SessionParser::new(Path::new("test-session"))
    }

    #[test]
    fn supports_old_shell_and_current_exec_arguments() {
        for (name, arguments) in [
            (
                "shell",
                json!({"command":["bash","-lc","cat ~/.ssh/id_rsa"]}),
            ),
            ("shell_command", json!({"command":"cat ~/.ssh/id_rsa"})),
            ("exec_command", json!({"cmd":"cat ~/.ssh/id_rsa"})),
            ("functions.exec_command", json!({"cmd":"cat ~/.ssh/id_rsa"})),
        ] {
            let events = parser().parse_line(&record(json!({"type":"function_call","name":name,"arguments":arguments.to_string(),"call_id":"call-1"})), GuardMode::Audit);
            assert_eq!(events.len(), 1, "{name}");
            assert_eq!(events[0].severity, "high");
        }
        let events = parser().parse_line(&record(json!({"type":"local_shell_call","call_id":"old","action":{"type":"exec","command":["echo","hello"]}})), GuardMode::Audit);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn paginated_code_mode_uses_completed_commands_without_duplicates() {
        let mut parser = parser();
        parser.read_metadata(&json!({"originator":"codex-tui","source":"cli","cli_version":"0.153.4","history_mode":"paginated"}));
        let call = record(
            json!({"type":"function_call","name":"exec_command","call_id":"call-1","arguments":"{\"cmd\":\"echo hello\"}"}),
        );
        assert!(parser.parse_line(&call, GuardMode::Audit).is_empty());
        let line = completed(
            json!({"type":"CommandExecution","id":"exec-1","command":["pwsh","-Command","$env:API_KEY='private-token'; Get-Content ~/.codex/auth.json"],"status":"completed","stdout":"secret output"}),
        );
        let events = parser.parse_line(&line, GuardMode::Audit);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source.as_deref(), Some("Codex CLI 0.153.4"));
        assert_eq!(events[0].severity, "high");
        let serialized = serde_json::to_string(&events).unwrap();
        assert!(!serialized.contains("private-token"));
        assert!(!serialized.contains("secret output"));
        assert!(parser.parse_line(&line, GuardMode::Audit).is_empty());
    }

    #[test]
    fn ignores_code_mode_source_and_unrelated_exec_tools() {
        for payload in [
            json!({"type":"custom_tool_call","name":"exec","input":"text(await tools.exec_command({cmd: 'echo hello'}));"}),
            json!({"type":"function_call","name":"mcp__server__exec_query","arguments":"{\"command\":\"not shell\"}"}),
            json!({"type":"function_call_output","output":"cat ~/.ssh/id_rsa"}),
        ] {
            assert!(parser()
                .parse_line(&record(payload), GuardMode::Audit)
                .is_empty());
        }
    }

    #[test]
    fn parallel_calls_have_distinct_ids_and_keep_each_command() {
        let payload = json!({"type":"function_call","name":"multi_tool_use.parallel","call_id":"parallel-1","arguments":{"tool_uses":[
            {"recipient_name":"functions.exec_command","parameters":{"cmd":"echo first"}},
            {"recipient_name":"functions.shell_command","parameters":{"command":"echo second"}}
        ]}});
        let events = parser().parse_line(&record(payload), GuardMode::Audit);
        assert_eq!(events.len(), 2);
        assert_ne!(events[0].id, events[1].id);
    }

    #[test]
    fn file_changes_record_counts_and_rename_without_file_contents() {
        let events = parser().parse_line(&completed(json!({"type":"FileChange","id":"patch-1","status":"completed","changes":{
            "src/new.txt":{"type":"add","content":"private-content\nsecond\n"},
            "src/old.txt":{"type":"delete","content":"old\n"},
            "src/main.rs":{"type":"update","move_path":"src/app.rs","unified_diff":"--- a/src/main.rs\n+++ b/src/app.rs\n@@ -1 +1,2 @@\n-old\n+new\n+more\n"}
        }})), GuardMode::Audit);
        assert_eq!(events.len(), 3);
        let updated = events
            .iter()
            .find(|event| event.kind == "file-modify")
            .unwrap();
        assert_eq!(updated.paths, ["src/main.rs", "src/app.rs"]);
        assert_eq!(updated.lines_added, Some(2));
        assert_eq!(updated.lines_removed, Some(1));
        assert_eq!(updated.line_delta, Some(1));
        assert!(!serde_json::to_string(&events)
            .unwrap()
            .contains("private-content"));
    }

    #[test]
    fn failed_patches_are_not_counted_as_completed_file_changes() {
        let events = parser().parse_line(&completed(json!({"type":"FileChange","id":"patch-failed","status":"failed","changes":{"~/.codex/auth.json":{"type":"delete","content":"secret"}}})), GuardMode::Audit);
        assert_eq!(events[0].kind, "command");
        assert_eq!(events[0].severity, "high");
        assert_eq!(events[0].lines_removed, None);
    }

    #[test]
    fn audits_interactive_input_and_legacy_completion_events() {
        let command = completed(
            json!({"type":"CommandExecution","id":"stdin-1","command":["pwsh"],"source":"unified_exec_interaction","interaction_input":"Get-Content ~/.ssh/id_rsa\n","status":"completed"}),
        );
        let events = parser().parse_line(&command, GuardMode::Audit);
        assert_eq!(events[0].severity, "high");
        assert!(events[0].command.as_ref().unwrap().contains("Get-Content"));
        let patch = json!({"type":"event_msg","payload":{"type":"patch_apply_end","call_id":"legacy-patch","success":true,"changes":{"src/new.txt":{"type":"add","content":"new\n"}}}});
        let events = parser().parse_line(patch.to_string().as_bytes(), GuardMode::Audit);
        assert_eq!(events[0].kind, "file-create");
        assert_eq!(events[0].lines_added, Some(1));
    }

    #[test]
    fn repeated_commands_without_call_ids_keep_distinct_timestamps() {
        let mut parser = parser();
        let mut record = json!({"timestamp":"2026-09-05T10:00:00Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":{"cmd":"echo repeated"}}});
        let first = parser.parse_line(record.to_string().as_bytes(), GuardMode::Audit);
        assert_eq!(first.len(), 1);
        assert!(parser
            .parse_line(record.to_string().as_bytes(), GuardMode::Audit)
            .is_empty());
        record["timestamp"] = json!("2026-09-05T10:00:01Z");
        let second = parser.parse_line(record.to_string().as_bytes(), GuardMode::Audit);
        assert_eq!(second.len(), 1);
        assert_ne!(first[0].id, second[0].id);
    }

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "codex-baoan-sessions-{}-{}",
                std::process::id(),
                Utc::now().timestamp_nanos_opt().unwrap()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn append(path: &Path, bytes: &[u8]) {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }

    #[test]
    fn follows_multiple_sessions_and_discovers_new_files() {
        let directory = TestDirectory::new();
        let first = directory.0.join("rollout-first.jsonl");
        let second = directory.0.join("rollout-second.jsonl");
        let line =
            completed(json!({"type":"CommandExecution","id":"same-id","command":["echo","hello"]}));
        append(&first, &[line.clone(), b"\n".to_vec()].concat());
        append(&second, &[line.clone(), b"\n".to_vec()].concat());
        let mut monitor = SessionMonitor::new(directory.0.clone());
        let events = monitor.poll(GuardMode::Audit);
        assert_eq!(events.len(), 2);
        assert_ne!(events[0].id, events[1].id);
        assert!(monitor.poll(GuardMode::Audit).is_empty());
        let third = directory.0.join("rollout-third.jsonl");
        append(&third, &[line, b"\n".to_vec()].concat());
        monitor.refresh();
        assert_eq!(monitor.poll(GuardMode::Audit).len(), 1);
    }

    #[test]
    fn waits_for_complete_utf8_records_and_recovers_after_truncation() {
        let directory = TestDirectory::new();
        let path = directory.0.join("rollout-partial.jsonl");
        let line = completed(
            json!({"type":"CommandExecution","id":"unicode","command":["echo","\u{4e2d}\u{6587}"]}),
        );
        let split = line.iter().position(|byte| *byte > 127).unwrap() + 1;
        append(&path, &line[..split]);
        let mut cursor = RolloutCursor::new(&path, 0);
        assert!(cursor.read(&path, GuardMode::Audit).is_empty());
        append(&path, &line[split..]);
        assert!(cursor.read(&path, GuardMode::Audit).is_empty());
        append(&path, b"\n");
        assert_eq!(cursor.read(&path, GuardMode::Audit).len(), 1);
        fs::write(&path, b"broken json\n").unwrap();
        assert!(cursor.read(&path, GuardMode::Audit).is_empty());
        append(&path, &[line, b"\n".to_vec()].concat());
        assert_eq!(cursor.read(&path, GuardMode::Audit).len(), 1);
    }

    #[test]
    fn reads_metadata_when_starting_from_tail() {
        let directory = TestDirectory::new();
        let path = directory.0.join("rollout-desktop.jsonl");
        let metadata = json!({"type":"session_meta","payload":{"originator":"Codex Desktop","source":"vscode","cli_version":"0.153.4","history_mode":"paginated"}}).to_string() + "\n";
        append(&path, metadata.as_bytes());
        let offset = metadata.len() as u64;
        let line =
            completed(json!({"type":"CommandExecution","id":"desktop","command":["echo","hello"]}));
        append(&path, &[line, b"\n".to_vec()].concat());
        let mut cursor = RolloutCursor::new(&path, offset);
        let events = cursor.read(&path, GuardMode::Audit);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source.as_deref(), Some("Codex Desktop 0.153.4"));
    }

    #[test]
    fn resumes_sessions_outside_the_initial_history_limit() {
        let directory = TestDirectory::new();
        let line =
            completed(json!({"type":"CommandExecution","id":"old","command":["echo","old"]}));
        for index in 0..=INITIAL_SESSIONS {
            append(
                &directory.0.join(format!("rollout-{index}.jsonl")),
                &[line.clone(), b"\n".to_vec()].concat(),
            );
        }
        let old_path = find_rollouts(&directory.0).last().unwrap().path.clone();
        let mut monitor = SessionMonitor::new(directory.0.clone());
        assert_eq!(monitor.poll(GuardMode::Audit).len(), INITIAL_SESSIONS);
        let new_line = completed(
            json!({"type":"CommandExecution","id":"resumed","command":["echo","resumed"]}),
        );
        append(&old_path, &[new_line, b"\n".to_vec()].concat());
        let events = monitor.poll(GuardMode::Audit);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].command.as_deref(), Some("echo resumed"));
    }

    #[test]
    fn tail_boundary_and_read_budget_do_not_drop_the_next_record() {
        let directory = TestDirectory::new();
        let path = directory.0.join("rollout-large.jsonl");
        append(&path, &vec![b'x'; READ_BYTES_PER_TICK as usize + 15]);
        append(&path, b"\n");
        let line = completed(
            json!({"type":"CommandExecution","id":"after-large","command":["echo","after-large"]}),
        );
        append(&path, &[line, b"\n".to_vec()].concat());
        let mut cursor = RolloutCursor::new(&path, 5);
        assert!(cursor.read(&path, GuardMode::Audit).is_empty());
        assert_eq!(cursor.read(&path, GuardMode::Audit).len(), 1);
        assert!(cursor.read(&path, GuardMode::Audit).is_empty());
    }

    #[test]
    #[ignore = "Read-only check of a local Codex rollout; opt in explicitly"]
    fn local_rollout_smoke() {
        let path = std::env::var_os("CODEX_BAOAN_SMOKE_ROLLOUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                find_rollouts(&super::super::codex_sessions_dir())
                    .remove(0)
                    .path
            });
        let mut parser = SessionParser::new(&path);
        let mut commands = 0;
        let mut files = 0;
        for line in BufReader::new(fs::File::open(&path).unwrap()).split(b'\n') {
            for event in parser.parse_line(&line.unwrap(), GuardMode::Audit) {
                commands += usize::from(event.command.is_some());
                files += usize::from(event.command.is_none());
            }
        }
        assert!(
            commands + files > 0,
            "No supported activity in the selected rollout"
        );
        println!(
            "source={}; paginated={}; commands={commands}; file_changes={files}",
            parser.source, parser.paginated
        );
    }
}
