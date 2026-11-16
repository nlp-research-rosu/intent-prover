//! Repository-root current specifications and immutable operation evidence.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const README: &str = "# .intent\n\nREQUIREMENTS.md holds project-wide human intent.\nsource/<source-path>/ holds requirements.json, index.json and current specs/<bundle-id>/ files.\nCross-file requirements live in source/requirements.json and have one owning per-source index.\nlogs/<operation-id>/ holds request.json, result.json, exact inputs/ and returned artifacts/.\nA saved operation is evidence of its recorded scope and outcome, not certification.\nprove/sessions/ contains local authoring scratch, not durable evidence.\nNOTES.md remains the project notes included by explicit Scan uploads.\n\nCommit requirements, current specifications and operation evidence with source.\nNever edit historical inputs/results or blanket-ignore .intent.\nThe local viewer excludes frozen input copies and authoring scratch.\nNo hooks, automatic reviews or background workers run.\n";

pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes.as_ref())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn valid_id(id: &str) -> bool {
    id.len() == 6
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

pub fn safe_parent(root: &Path, file: &Path) -> Result<()> {
    let base = intent(root)?;
    let relative = file.strip_prefix(&base)?;
    let mut current = base;
    for part in relative.components() {
        ensure!(
            matches!(part, std::path::Component::Normal(_)),
            "Unsafe evidence path"
        );
        current.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            ensure!(!metadata.is_symlink(), "Evidence must not follow symlinks");
        }
    }
    Ok(())
}

pub fn intent(root: &Path) -> Result<PathBuf> {
    let dir = root.join(".intent");
    if let Ok(metadata) = fs::symlink_metadata(&dir) {
        ensure!(
            metadata.is_dir() && !metadata.is_symlink(),
            ".intent must be a real directory"
        );
    }
    Ok(dir)
}

pub fn initialize(root: &Path) -> Result<PathBuf> {
    let dir = intent(root)?;
    fs::create_dir_all(&dir)?;
    for (name, text) in [
        ("REQUIREMENTS.md", crate::requirements::BRIEF),
        ("README.md", README),
        (".ignore", "*\n"),
        (".gitignore", ".sent.log\n.storage.lock\n"),
    ] {
        let file = dir.join(name);
        safe_parent(root, &file)?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
        {
            Ok(mut output) => {
                use std::io::Write;
                output.write_all(text.as_bytes())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(dir)
}

pub fn lock(root: &Path) -> Result<fs::File> {
    let file = initialize(root)?.join(".storage.lock");
    safe_parent(root, &file)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(file)?;
    file.lock()?;
    Ok(file)
}

/// Freeze selected inputs and their local associations before a remote submission.
pub fn operation(root: &Path, mut request: Value, files: &[(String, Vec<u8>)]) -> Result<PathBuf> {
    let _lock = lock(root)?;
    let id = new_id(root)?;
    let evidence = root.join(".intent/logs").join(&id);
    safe_parent(root, &evidence)?;
    fs::create_dir_all(evidence.parent().context("Missing logs directory")?)?;
    fs::create_dir(&evidence)?;
    fs::create_dir(evidence.join("inputs"))?;
    fs::create_dir(evidence.join("artifacts"))?;
    let ids = request["requirements"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let specification = specification(root, &ids, request["spec_id"].as_str())?;
    request["properties"] = property_snapshot(root, &ids, files, &specification)?;
    request["specification"] = specification;
    request
        .as_object_mut()
        .context("Invalid operation request")?
        .remove("spec_id");
    let mut manifest = Vec::new();
    for (name, bytes) in files {
        crate::requirements::relative_path(name)?;
        let file = evidence.join("inputs").join(name);
        safe_parent(root, &file)?;
        fs::create_dir_all(file.parent().context("Missing input parent")?)?;
        fs::write(&file, bytes)?;
        manifest.push(json!({"path":format!("logs/{id}/inputs/{name}"),"sha256":hash(bytes)}));
    }
    request["version"] = json!(1);
    request["operation_id"] = json!(id);
    request["inputs"] = json!(manifest);
    request["submitted_at"] = json!(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    );
    crate::api::write_json(&evidence, "request.json", &request)?;
    Ok(evidence)
}

/// Allocate a six-character identity while the caller holds the storage lock.
pub fn new_id(root: &Path) -> Result<String> {
    let mut used = BTreeSet::new();
    for (_, _, doc) in crate::requirements::documents(root)? {
        used.extend(doc.requirements.into_iter().map(|r| r.id));
    }
    for (_, index) in indexes(root)? {
        for (id, selection) in index.properties {
            used.insert(id);
            used.extend(selection.last_run);
        }
    }
    used.extend(specifications(root)?.into_keys());
    let logs = root.join(".intent/logs");
    safe_parent(root, &logs)?;
    if logs.exists() {
        for entry in fs::read_dir(logs)? {
            let entry = entry?;
            used.insert(entry.file_name().to_string_lossy().into_owned());
            let request = entry.path().join("request.json");
            safe_parent(root, &request)?;
            if request.is_file() {
                let request: Value = serde_json::from_slice(&fs::read(request)?)?;
                if let Some(id) = request["specification"]["id"].as_str() {
                    used.insert(id.into());
                }
                for requirement in request["requirements"].as_array().into_iter().flatten() {
                    if let Some(id) = requirement["id"].as_str() {
                        used.insert(id.into());
                    }
                }
            }
        }
    }
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    loop {
        let random = uuid::Uuid::new_v4();
        let bytes = random.as_bytes();
        let mut value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if value >= 36_u32.pow(6) {
            continue;
        }
        let mut characters = [b'a'; 6];
        for character in characters.iter_mut().rev() {
            *character = ALPHABET[(value % 36) as usize];
            value /= 36;
        }
        let id: String = characters.into_iter().map(char::from).collect();
        if !used.contains(&id) {
            return Ok(id);
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub specs: Vec<String>,
    pub last_run: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub schema_version: u32,
    pub properties: BTreeMap<String, Selection>,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            schema_version: 1,
            properties: BTreeMap::new(),
        }
    }
}

pub fn source_dir(root: &Path, source: &str) -> Result<PathBuf> {
    crate::requirements::relative_path(source)?;
    let path = root.join(".intent/source").join(source);
    safe_parent(root, &path)?;
    Ok(path)
}

pub fn read_index(root: &Path, source: &str) -> Result<Index> {
    let file = source_dir(root, source)?.join("index.json");
    safe_parent(root, &file)?;
    if !file.exists() {
        return Ok(Index::default());
    }
    let index: Index = serde_json::from_slice(&fs::read(file)?)?;
    ensure!(
        index.schema_version == 1,
        "Unsupported index schema version"
    );
    for (id, selection) in &index.properties {
        ensure!(valid_id(id), "Invalid property ID: {id}");
        ensure!(
            selection.last_run.as_deref().is_none_or(valid_id),
            "Invalid operation ID"
        );
        for spec in &selection.specs {
            spec_path(root, spec)?;
        }
    }
    Ok(index)
}

pub fn write_index(root: &Path, source: &str, index: &Index) -> Result<()> {
    let dir = source_dir(root, source)?;
    safe_parent(root, &dir.join("index.json"))?;
    fs::create_dir_all(&dir)?;
    crate::api::write_json(&dir, "index.json", &serde_json::to_value(index)?)
}

pub fn spec_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let (source, name) = relative
        .strip_prefix("source/")
        .and_then(|p| p.split_once("/specs/"))
        .context("Expected a per-source specification path")?;
    crate::requirements::relative_path(source)?;
    crate::requirements::relative_path(name)?;
    ensure!(
        name.split_once('/').is_some_and(|(id, _)| valid_id(id)),
        "Expected specs/<six-character-id>/<file>"
    );
    let file = root.join(".intent").join(relative);
    safe_parent(root, &file)?;
    Ok(file)
}

pub fn indexes(root: &Path) -> Result<Vec<(String, Index)>> {
    fn visit(root: &Path, dir: &Path, out: &mut Vec<(String, Index)>) -> Result<()> {
        safe_parent(root, dir)?;
        if !dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path();
            if path.join("index.json").exists() {
                let source = path
                    .strip_prefix(root.join(".intent/source"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((source.clone(), read_index(root, &source)?));
            } else if !path.join("requirements.json").exists() {
                visit(root, &path, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(root, &root.join(".intent/source"), &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    let mut owners = BTreeSet::new();
    for (_, index) in &out {
        for id in index.properties.keys() {
            ensure!(
                owners.insert(id),
                "Property has more than one owning index: {id}"
            );
        }
    }
    Ok(out)
}

fn specifications(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (source, _) in indexes(root)? {
        let dir = source_dir(root, &source)?.join("specs");
        safe_parent(root, &dir)?;
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            ensure!(valid_id(&id), "Invalid specification ID: {id}");
            ensure!(
                out.insert(id.clone(), format!("source/{source}/specs/{id}"))
                    .is_none(),
                "Duplicate specification ID: {id}"
            );
        }
    }
    Ok(out)
}

pub fn specification(root: &Path, ids: &[Value], existing: Option<&str>) -> Result<Value> {
    if ids.is_empty() {
        ensure!(existing.is_none(), "--spec-id requires --requirement");
        return Ok(Value::Null);
    }
    if let Some(id) = existing {
        ensure!(valid_id(id), "Expected a six-character specification ID");
        let path = specifications(root)?
            .remove(id)
            .context("Unknown specification ID")?;
        return Ok(json!({"id":id,"path":path}));
    }
    let id = new_id(root)?;
    let property = ids[0]["id"].as_str().context("Missing property ID")?;
    let owner = indexes(root)?
        .into_iter()
        .find_map(|(owner, index)| index.properties.contains_key(property).then_some(owner))
        .context("Property has no owning index")?;
    Ok(json!({"id":id,"path":format!("source/{owner}/specs/{id}")}))
}

pub fn property_placement(
    root: &Path,
    id: &str,
    targets: &[crate::requirements::Target],
) -> Result<Vec<(String, Index)>> {
    ensure!(valid_id(id), "Invalid property ID");
    let mut selection = Selection::default();
    let mut owner = None;
    let indexes = indexes(root)?;
    for (source, index) in &indexes {
        if let Some(saved) = index.properties.get(id) {
            selection = saved.clone();
            if targets.iter().any(|target| target.path == *source) {
                owner = Some(source.clone());
            }
        }
    }
    let owner = owner.unwrap_or_else(|| targets[0].path.clone());
    let mut changes = Vec::new();
    for (source, mut index) in indexes {
        if source != owner && index.properties.remove(id).is_some() {
            changes.push((source, index));
        }
    }
    let mut index = read_index(root, &owner)?;
    index.properties.insert(id.into(), selection);
    changes.push((owner, index));
    Ok(changes)
}

pub fn property_removal(root: &Path, id: &str) -> Result<Vec<(String, Index)>> {
    ensure!(valid_id(id), "Invalid property ID");
    let mut changes = Vec::new();
    for (source, mut index) in indexes(root)? {
        if index.properties.remove(id).is_some() {
            changes.push((source, index));
        }
    }
    Ok(changes)
}

pub fn property_snapshot(
    root: &Path,
    ids: &[Value],
    inputs: &[(String, Vec<u8>)],
    specification: &Value,
) -> Result<Value> {
    let docs = crate::requirements::documents(root)?;
    let indexes = indexes(root)?;
    let mut properties = Vec::new();
    for bound in ids {
        let id = bound["id"].as_str().context("Missing property ID")?;
        let record = docs
            .iter()
            .flat_map(|(_, _, doc)| &doc.requirements)
            .find(|r| r.id == id)
            .context("Unknown property")?;
        let (owner, index) = indexes
            .iter()
            .find(|(_, index)| index.properties.contains_key(id))
            .context("Property has no owning index")?;
        let selection = &index.properties[id];
        let sources: Vec<_> = record
            .targets
            .iter()
            .map(|target| {
                let file = root.join(&target.path).canonicalize()?;
                ensure!(
                    file.starts_with(root),
                    "Property source is outside the repository"
                );
                Ok(json!({"path":target.path,"sha256":hash(fs::read(file)?)}))
            })
            .collect::<Result<_>>()?;
        let mut specs = BTreeMap::new();
        for path in &selection.specs {
            specs.insert(
                path.clone(),
                fs::read(spec_path(root, path)?).ok().map(hash),
            );
        }
        let mut outputs = Vec::new();
        for (name, _) in inputs.iter().filter(|(name, _)| name.ends_with(".k")) {
            let directory = specification["path"]
                .as_str()
                .context("Missing specification identity")?;
            let path = format!("{directory}/{name}");
            let file = spec_path(root, &path)?;
            specs
                .entry(path.clone())
                .or_insert_with(|| fs::read(file).ok().map(hash));
            outputs.push(json!({"input":name,"path":path}));
        }
        properties.push(json!({"id":id,"owner":owner,"selection":selection,"record_sha256":bound["record_sha256"],"sources":sources,"specs":specs,"outputs":outputs}));
    }
    Ok(json!(properties))
}

/// Link only formal versions from this operation's exact submission manifest.
pub fn finish_properties(root: &Path, evidence: &Path, successful: bool) -> Result<()> {
    let _lock = lock(root)?;
    safe_parent(root, &evidence.join("request.json"))?;
    safe_parent(root, &evidence.join("result.json"))?;
    let request: Value = serde_json::from_slice(&fs::read(evidence.join("request.json"))?)?;
    let run = evidence
        .file_name()
        .and_then(|s| s.to_str())
        .context("Missing operation ID")?;
    ensure!(valid_id(run), "Invalid operation ID");
    let docs = crate::requirements::documents(root)?;
    let mut result: Value = serde_json::from_slice(&fs::read(evidence.join("result.json"))?)?;
    let mut written = BTreeMap::<String, String>::new();
    for property in request["properties"].as_array().into_iter().flatten() {
        let id = property["id"].as_str().context("Missing property ID")?;
        let owner = property["owner"].as_str().context("Missing owner")?;
        let mut index = read_index(root, owner)?;
        let Some(selection) = index.properties.get_mut(id) else {
            continue;
        };
        if selection.last_run.as_deref() == Some(run)
            || serde_json::to_value(&*selection)? != property["selection"]
        {
            continue;
        }
        let record = docs
            .iter()
            .flat_map(|(_, _, doc)| &doc.requirements)
            .find(|r| r.id == id);
        if record
            .map(crate::requirements::record_hash)
            .transpose()?
            .as_deref()
            != property["record_sha256"].as_str()
        {
            continue;
        }
        let sources_match = property["sources"].as_array().is_some_and(|sources| {
            !sources.is_empty()
                && sources.iter().all(|source| {
                    source["path"]
                        .as_str()
                        .and_then(|p| fs::read(root.join(p)).ok())
                        .map(hash)
                        .as_deref()
                        == source["sha256"].as_str()
                })
        });
        let specs_match = property["specs"].as_object().is_some_and(|specs| {
            specs.iter().all(|(path, expected)| {
                let actual = spec_path(root, path)
                    .ok()
                    .and_then(|p| fs::read(p).ok())
                    .map(hash);
                actual.as_deref() == expected.as_str()
                    || written
                        .get(path)
                        .is_some_and(|hash| Some(hash) == actual.as_ref())
            })
        });
        let inputs_match = request["inputs"].as_array().is_some_and(|inputs| {
            inputs.iter().all(|input| {
                input["path"]
                    .as_str()
                    .and_then(|path| {
                        let file = root.join(".intent").join(path);
                        safe_parent(root, &file).ok()?;
                        fs::read(file).ok()
                    })
                    .map(hash)
                    .as_deref()
                    == input["sha256"].as_str()
            })
        });
        if !sources_match || !specs_match || !inputs_match {
            continue;
        }
        let mut submitted = Vec::new();
        for output in property["outputs"].as_array().into_iter().flatten() {
            let name = output["input"].as_str().context("Missing formal input")?;
            crate::requirements::relative_path(name)?;
            let frozen = format!("logs/{run}/inputs/{name}");
            let input = request["inputs"]
                .as_array()
                .context("Missing submitted inputs")?
                .iter()
                .find(|input| input["path"] == frozen)
                .context("Formal input is absent from the submission manifest")?;
            let sha256 = input["sha256"]
                .as_str()
                .context("Missing submitted input hash")?;
            let path = output["path"]
                .as_str()
                .context("Missing current specification path")?;
            submitted.push(json!({"path":path,"sha256":sha256}));
            if successful {
                let bytes = fs::read(evidence.join("inputs").join(name))?;
                let artifact = evidence.join("artifacts").join(name);
                safe_parent(root, &artifact)?;
                fs::create_dir_all(artifact.parent().context("Missing artifact parent")?)?;
                fs::write(artifact, &bytes)?;
                let file = spec_path(root, path)?;
                fs::create_dir_all(file.parent().context("Missing specification parent")?)?;
                let temporary = file.with_file_name(format!(".{}.tmp", uuid::Uuid::new_v4()));
                written.insert(path.into(), hash(&bytes));
                fs::write(&temporary, bytes)?;
                fs::rename(temporary, file)?;
                if !selection.specs.iter().any(|p| p == path) {
                    selection.specs.push(path.into());
                }
            }
        }
        selection.specs.sort();
        selection.last_run = Some(run.into());
        if !result["properties"].is_object() {
            result["properties"] = json!({});
        }
        result["properties"][id.to_string()] = json!({"specs":submitted});
        crate::api::write_json(evidence, "result.json", &result)?;
        write_index(root, owner, &index)?;
    }
    Ok(())
}
