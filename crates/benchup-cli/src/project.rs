//! A `.bup` file opened for tool calls. Every change is recorded in the
//! document history (so BenchUp can undo it) and saved atomically.
use benchup_core::{Document, Model};
use benchup_tools::batch::{Applied, report, resolve};
use benchup_tools::{Call, ToolEnv, execute, files, is_mutating};
use serde_json::{Value, json};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

/// History label of agent changes, the English source of the `cli.agent_edit`
/// message; the app shows it in the user's language, as it does its own labels.
pub const HISTORY_LABEL: &str = "Agent edit";

pub const CHANGED_ON_DISK: &str =
    "The project file changed on disk since it was opened (was it saved in BenchUp?). Run the command again.";

/// Content fingerprint, to refuse overwriting a file somebody else changed.
fn stamp(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

pub struct Project {
    path: PathBuf,
    allow_shared: bool,
    pub doc: Document,
    env: ToolEnv,
    stamp: u64,
    counter: u64,
    /// The file as last read or written; a PNG preview is kept when saving.
    saved: Vec<u8>,
}

impl Project {
    pub fn open(path: impl Into<PathBuf>, allow_shared: bool) -> Result<Self, String> {
        let path = path.into();
        let bytes = std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let doc = load(&path, &bytes, allow_shared)?;
        let env = ToolEnv {
            catalog: doc.model.stock_materials.clone(),
            selection: doc.selection.clone(),
            ..ToolEnv::default()
        };
        Ok(Self { path, allow_shared, doc, env, stamp: stamp(&bytes), counter: 0, saved: bytes })
    }

    /// Reads the file again if it changed on disk since it was last read or
    /// written (saved in BenchUp while an MCP session has it open). The open
    /// group stays open when it still exists.
    pub fn refresh(&mut self) -> Result<(), String> {
        let bytes = std::fs::read(&self.path).map_err(|e| format!("cannot read {}: {e}", self.path.display()))?;
        if stamp(&bytes) == self.stamp {
            return Ok(());
        }
        let mut doc = load(&self.path, &bytes, self.allow_shared)?;
        doc.model.edit_context = self.doc.model.edit_context.filter(|id| doc.model.objects.contains_key(id));
        self.env.catalog = doc.model.stock_materials.clone();
        self.env.selection = doc.selection.clone();
        self.doc = doc;
        self.stamp = stamp(&bytes);
        self.saved = bytes;
        Ok(())
    }

    /// Writes a new, empty project. Refuses to replace an existing file unless `force`.
    pub fn create(path: &Path, name: Option<&str>, force: bool) -> Result<Document, String> {
        if path.exists() && !force {
            return Err(format!("{} already exists (pass --force to replace it)", path.display()));
        }
        let mut doc = Document::default();
        doc.name = match name {
            Some(name) => name.to_owned(),
            None => path.file_stem().map_or(doc.name.clone(), |s| s.to_string_lossy().into_owned()),
        };
        files::save_atomic(path, &files::encode_document(&doc)?)?;
        Ok(doc)
    }

    pub fn save(&mut self) -> Result<(), String> {
        let current = std::fs::read(&self.path).map_err(|_| CHANGED_ON_DISK.to_owned())?;
        if stamp(&current) != self.stamp {
            return Err(CHANGED_ON_DISK.to_owned());
        }
        let bytes = files::reencode_like(&self.saved, &self.doc)?;
        files::save_atomic(&self.path, &bytes)?;
        self.stamp = stamp(&bytes);
        self.saved = bytes;
        Ok(())
    }

    /// Applies `calls` in order as one all-or-nothing step: if any call fails,
    /// the document is left as it was. Changes become a single undo entry.
    pub fn apply(&mut self, calls: &[(String, Value)]) -> Result<Applied, String> {
        self.run(calls, true)
    }

    /// Like `apply`, but `save == false` leaves the file and the document as
    /// they were (a dry run) while still reporting every result.
    pub fn run(&mut self, calls: &[(String, Value)], save: bool) -> Result<Applied, String> {
        // Only changing tools touch more of the model than the open group, so
        // reading calls skip the copy needed to roll back.
        let model_before = calls.iter().any(|(name, _)| is_mutating(name)).then(|| self.doc.model.clone());
        let context_before = self.doc.model.edit_context;
        let selection_before = self.env.selection.clone();
        self.env.before = None;
        self.env.created.clear();
        // `$N` references only mean something in a batch; a single call's text is taken as written.
        let batch = calls.len() > 1;
        let mut results = Vec::with_capacity(calls.len());
        let mut mutated = false;
        for (index, (name, arguments)) in calls.iter().enumerate() {
            self.counter += 1;
            let arguments = if batch { resolve(arguments, &results) } else { Ok(arguments.clone()) };
            let arguments = match arguments {
                Ok(arguments) => arguments,
                Err(error) => {
                    self.restore(model_before, context_before, selection_before);
                    return Ok(Applied::Failed {
                        index,
                        earlier: results,
                        result: json!({"ok": false, "error": error,
                            "hint": "Refer to an earlier call as \"$N.field\" (N counts calls from 1), e.g. \"$1.created[0]\" or \"$3.joint_id\"."}),
                    });
                }
            };
            let call = Call::new(self.counter.to_string(), name, arguments);
            let outcome = execute(&mut self.doc.model, &call, &mut self.env);
            if !outcome.ok {
                self.restore(model_before, context_before, selection_before);
                return Ok(Applied::Failed { index, result: outcome.result, earlier: results });
            }
            mutated |= outcome.mutated;
            let mut result = outcome.result;
            if name == "get_scene" {
                result["project"] = json!({"name": self.doc.name, "file": self.path.display().to_string()});
            }
            results.push(result);
        }
        if !save {
            self.restore(model_before, context_before, selection_before);
            return Ok(Applied::Ok(results));
        }
        if let Some(before) = model_before.filter(|_| mutated) {
            self.doc.history.record(HISTORY_LABEL, before, &self.doc.model);
        }
        let selection_changed = self.env.selection != selection_before;
        self.doc.selection = self.env.selection.clone();
        if (mutated || selection_changed)
            && let Err(error) = self.save()
        {
            // Back to the file as last read or written, which is this
            // document before the calls (history included).
            self.doc = files::decode(&self.saved)?;
            self.doc.model.edit_context = context_before;
            self.env.selection = selection_before;
            return Err(error);
        }
        Ok(Applied::Ok(results))
    }

    /// Undoes the calls of a failed or dry run.
    fn restore(&mut self, model: Option<Model>, context: Option<u64>, selection: std::collections::BTreeSet<u64>) {
        if let Some(model) = model {
            self.doc.model = model;
        }
        self.doc.model.edit_context = context;
        self.env.selection = selection;
    }
}

/// The document stored in `bytes`, refusing a shared project unless allowed.
fn load(path: &Path, bytes: &[u8], allow_shared: bool) -> Result<Document, String> {
    let doc = files::decode(bytes).map_err(|e| format!("cannot load {}: {e}", path.display()))?;
    if doc.shared_project.is_some() && !allow_shared {
        return Err(format!(
            "{} is a shared project; edits made here would diverge from the server. \
             Pass --allow-shared to edit the local copy anyway.",
            path.display()
        ));
    }
    Ok(doc)
}

impl Project {
    /// Undo (`redo == false`) or redo the last history entry and save.
    pub fn step_history(&mut self, redo: bool) -> Result<Value, String> {
        let label = if redo { self.doc.history.redo_label() } else { self.doc.history.undo_label() }
            .map(str::to_owned);
        let changed =
            if redo { self.doc.history.redo(&mut self.doc.model) } else { self.doc.history.undo(&mut self.doc.model) };
        if !changed {
            let what = if redo { "redo" } else { "undo" };
            return Ok(json!({"ok": false, "error": format!("nothing to {what}")}));
        }
        // Undo may remove entities the saved selection still points at.
        self.doc.selection.retain(|id| self.doc.model.contains_entity(id));
        self.env.selection = self.doc.selection.clone();
        self.save()?;
        let key = if redo { "redone" } else { "undone" };
        let mut result = json!({"ok": true});
        result[key] = json!(label);
        Ok(result)
    }
}

/// MCP: each `tools/call` is applied (and saved) on its own, on the file as
/// it is now on disk.
impl crate::mcp::Backend for Project {
    fn call(&mut self, _client: &str, name: &str, arguments: Value) -> (Value, bool) {
        if let Err(error) = self.refresh() {
            return (json!({"ok": false, "error": error}), false);
        }
        match self.apply(&[(name.to_owned(), arguments)]) {
            Ok(Applied::Ok(mut results)) => (results.pop().unwrap_or(Value::Null), true),
            Ok(Applied::Failed { result, .. }) => (result, false),
            Err(error) => (json!({"ok": false, "error": error}), false),
        }
    }

    fn batch(&mut self, calls: Vec<(String, Value)>, dry_run: bool) -> (Value, bool) {
        if let Err(error) = self.refresh() {
            return (json!({"ok": false, "error": error}), false);
        }
        match self.run(&calls, !dry_run) {
            Ok(applied) => {
                let (mut value, ok) = report(&calls, applied, true);
                if dry_run {
                    value["dry_run"] = json!(true);
                }
                (value, ok)
            }
            Err(error) => (json!({"ok": false, "error": error}), false),
        }
    }

    fn history(&mut self, redo: bool) -> (Value, bool) {
        if let Err(error) = self.refresh() {
            return (json!({"ok": false, "error": error}), false);
        }
        match self.step_history(redo) {
            Ok(value) => {
                let ok = value["ok"] == true;
                (value, ok)
            }
            Err(error) => (json!({"ok": false, "error": error}), false),
        }
    }

    fn render(&mut self, view: &str, width: u32, height: u32) -> Result<(String, Value), String> {
        self.refresh()?;
        let rendered = benchup_tools::render::render(&self.doc.model, view, width, height)?;
        Ok((benchup_tools::render::base64(&rendered.png), rendered.info))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_to_overwrite_a_file_changed_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.bup");
        Project::create(&path, Some("P"), false).unwrap();
        let mut project = Project::open(&path, false).unwrap();
        // BenchUp saves the project while this session has it open.
        let other = Document { name: "Saved elsewhere".into(), ..Document::default() };
        files::save_atomic(&path, &files::encode_document(&other).unwrap()).unwrap();

        let board = json!({"name": "B", "origin_mm": [0, 0, 0], "size_mm": [10, 10, 10]});
        let error = project.apply(&[("create_board".into(), board)]).err().unwrap();
        assert_eq!(error, CHANGED_ON_DISK);
        assert_eq!(files::decode(&std::fs::read(&path).unwrap()).unwrap().name, "Saved elsewhere");
        assert!(project.doc.model.root_entities().is_empty(), "the in-memory change is rolled back");
    }

    #[test]
    fn mcp_calls_work_on_the_file_as_saved_elsewhere() {
        use crate::mcp::Backend;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.bup");
        Project::create(&path, Some("P"), false).unwrap();
        let mut project = Project::open(&path, false).unwrap();
        let other = Document { name: "Saved elsewhere".into(), ..Document::default() };
        files::save_atomic(&path, &files::encode_document(&other).unwrap()).unwrap();

        let board = json!({"name": "B", "origin_mm": [0, 0, 0], "size_mm": [10, 10, 10]});
        let (result, ok) = project.call("agent", "create_board", board);
        assert!(ok, "{result}");
        let saved = files::decode(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.name, "Saved elsewhere", "the outside save is kept");
        assert_eq!(saved.model.root_entities().len(), 1);
        let (scene, _) = project.call("agent", "get_scene", json!({}));
        assert_eq!(scene["project"]["name"], "Saved elsewhere");
    }

    #[test]
    fn a_single_call_takes_dollar_text_as_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.bup");
        Project::create(&path, Some("P"), false).unwrap();
        let mut project = Project::open(&path, false).unwrap();
        let board = json!({"name": "$1", "origin_mm": [0, 0, 0], "size_mm": [10, 10, 10]});
        assert!(matches!(project.apply(&[("create_board".into(), board)]).unwrap(), Applied::Ok(_)));
        let doc = files::decode(&std::fs::read(&path).unwrap()).unwrap();
        assert!(doc.model.objects.values().any(|o| o.name == "$1"));
        assert_eq!(doc.history.labels().collect::<Vec<_>>(), [HISTORY_LABEL]);
    }
}
