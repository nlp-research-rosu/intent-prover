use crate::{CommandResult, config, semantics};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub session_id: Uuid,
    pub project_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_id: Option<String>,
    pub semantics: semantics::Identity,
    pub attempts_used: u32,
    pub validations_used: u32,
    #[serde(default)]
    pub runs_used: u32,
}

pub fn directory(id: Uuid) -> Result<PathBuf> {
    Ok(config::directory()?.join("sessions").join(id.to_string()))
}

pub fn workspace(session: &Session) -> PathBuf {
    match &session.log_id {
        Some(id) => session.project_root.join(".intent/logs").join(id),
        None => session
            .project_root
            .join(".intent/prove/sessions")
            .join(session.session_id.to_string()),
    }
}

pub fn legacy_operation(name: &str) -> Option<&str> {
    let (phase, number) = name.split_once('-')?;
    number.parse::<u32>().ok()?;
    match phase {
        "proof" => Some("prove"),
        "validation" => Some("validate"),
        "run" => Some("run"),
        _ => None,
    }
}

pub fn start(semantics_id: &str) -> CommandResult {
    let project_root = project_root(&std::env::current_dir()?)?;
    let semantics = semantics::select(semantics_id, &config::load()?)?;
    let _lock = crate::storage::lock(&project_root)?;
    let log_id = crate::storage::new_id(&project_root)?;
    let session = Session {
        session_id: Uuid::new_v4(),
        project_root,
        log_id: Some(log_id),
        semantics,
        attempts_used: 0,
        validations_used: 0,
        runs_used: 0,
    };
    fs::create_dir_all(directory(session.session_id)?.parent().unwrap())?;
    fs::create_dir(directory(session.session_id)?)?;
    crate::storage::safe_parent(&session.project_root, &workspace(&session))?;
    fs::create_dir_all(
        workspace(&session)
            .parent()
            .context("Missing logs directory")?,
    )?;
    fs::create_dir(workspace(&session))?;
    save(&session)?;
    Ok((0, view(&session)?))
}

pub fn load(id: Uuid) -> Result<Session> {
    let session: Session = serde_json::from_slice(
        &fs::read(directory(id)?.join("session.json")).context("Unknown session ID")?,
    )?;
    ensure!(
        session.session_id == id,
        "Session ID does not match its directory"
    );
    ensure!(
        session
            .log_id
            .as_deref()
            .is_none_or(crate::storage::valid_id),
        "Invalid session log ID"
    );
    Ok(session)
}

pub fn save(session: &Session) -> Result<()> {
    crate::api::write_json(
        &directory(session.session_id)?,
        "session.json",
        &serde_json::to_value(session)?,
    )
}

pub fn open(id: Uuid) -> Result<(Session, fs::File)> {
    let directory = directory(id)?;
    ensure!(directory.is_dir(), "Unknown session ID");
    let lock = lock(&directory)?;
    let session = load(id)?;
    ensure!(
        session.project_root.is_dir(),
        "Session project no longer exists"
    );
    Ok((session, lock))
}

pub fn project_root(directory: &Path) -> Result<PathBuf> {
    let directory = directory.canonicalize()?;
    // .git is a directory in ordinary repositories and a file in worktrees and submodules.
    // Without a repository, retain the supplied directory as the project root.
    Ok(directory
        .ancestors()
        .find(|ancestor| {
            let git = ancestor.join(".git");
            git.is_dir() || git.is_file()
        })
        .unwrap_or(&directory)
        .to_path_buf())
}

pub fn lock(directory: &Path) -> Result<fs::File> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("lock"))?;
    file.lock()?;
    Ok(file)
}

pub fn view(session: &Session) -> Result<Value> {
    let mut value = serde_json::to_value(session)?;
    value["workspaceDir"] = json!(workspace(session));
    Ok(value)
}

pub fn budget(used: u32, limit: u32) -> Value {
    json!({"used": used, "limit": limit, "remaining": limit.saturating_sub(used)})
}
