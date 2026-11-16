use crate::{
    CommandResult, ProofArgs, ProveArgs, RunArgs, TaskCommand, api::Api, channel, config, session,
};
use anyhow::{bail, ensure};
use serde_json::{Value, json};

pub fn run_program(args: RunArgs) -> CommandResult {
    let id = args.session;
    let (proof_session, lock) = session::open(id)?;
    let config = config::load()?;
    if proof_session.runs_used >= config.max_run_attempts {
        return exhausted("run", proof_session.runs_used, config.max_run_attempts);
    }
    let api = Api::new(&config)?;
    drop(lock);
    let upload = api.prepare_run(&proof_session.project_root, args)?;
    let (mut proof_session, lock) = session::open(id)?;
    if proof_session.runs_used >= config.max_run_attempts {
        return exhausted("run", proof_session.runs_used, config.max_run_attempts);
    }
    let evidence = api.record_request(&upload, &proof_session, "run")?;
    let response = api
        .submit_run(&upload)
        .map_err(|error| submission_failure(&evidence, error))?;
    proof_session.runs_used += 1;
    session::save(&proof_session)?;
    api.record_submission(&response, &evidence)?;
    drop(lock);
    let task = api.complete(
        &response,
        config.poll_interval_seconds,
        evidence.clone(),
        &proof_session,
    )?;
    finish(
        &proof_session,
        task,
        evidence,
        session::budget(proof_session.runs_used, config.max_run_attempts),
    )
}

pub fn validate(args: ProofArgs) -> CommandResult {
    let id = args.session;
    let (proof_session, lock) = session::open(id)?;
    let root = session::workspace(&proof_session);
    let config = config::load()?;
    if proof_session.validations_used >= config.max_validation_attempts {
        return exhausted(
            "validation",
            proof_session.validations_used,
            config.max_validation_attempts,
        );
    }
    let api = Api::new(&config)?;
    drop(lock);
    let upload = api.prepare_proof(&root, args, None)?;
    let (mut proof_session, lock) = session::open(id)?;
    if proof_session.validations_used >= config.max_validation_attempts {
        return exhausted(
            "validation",
            proof_session.validations_used,
            config.max_validation_attempts,
        );
    }
    let evidence = api.record_request(&upload, &proof_session, "validate")?;
    let response = api
        .submit_validation(&upload)
        .map_err(|error| submission_failure(&evidence, error))?;
    proof_session.validations_used += 1;
    session::save(&proof_session)?;
    api.record_submission(&response, &evidence)?;
    drop(lock);
    let task = api.complete(
        &response,
        config.poll_interval_seconds,
        evidence.clone(),
        &proof_session,
    )?;
    finish(
        &proof_session,
        task,
        evidence,
        session::budget(
            proof_session.validations_used,
            config.max_validation_attempts,
        ),
    )
}

pub fn prove(args: ProveArgs) -> CommandResult {
    let id = args.proof.session;
    let (proof_session, lock) = session::open(id)?;
    let root = session::workspace(&proof_session);
    let config = config::load()?;
    if proof_session.attempts_used >= config.max_proof_attempts {
        return exhausted(
            "proof",
            proof_session.attempts_used,
            config.max_proof_attempts,
        );
    }
    let api = Api::new(&config)?;
    drop(lock);
    let upload = api.prepare_proof(&root, args.proof, args.depth)?;
    let (mut proof_session, lock) = session::open(id)?;
    if proof_session.attempts_used >= config.max_proof_attempts {
        return exhausted(
            "proof",
            proof_session.attempts_used,
            config.max_proof_attempts,
        );
    }
    let evidence = api.record_request(&upload, &proof_session, "prove")?;
    let response = api
        .submit_proof(&upload)
        .map_err(|error| submission_failure(&evidence, error))?;
    proof_session.attempts_used += 1;
    session::save(&proof_session)?;
    api.record_submission(&response, &evidence)?;
    drop(lock);
    let task = api.complete(
        &response,
        config.poll_interval_seconds,
        evidence.clone(),
        &proof_session,
    )?;
    finish(
        &proof_session,
        task,
        evidence,
        session::budget(proof_session.attempts_used, config.max_proof_attempts),
    )
}

pub fn task(command: TaskCommand) -> CommandResult {
    let config = config::load()?;
    let api = Api::new(&config)?;
    match command {
        TaskCommand::List {
            status,
            before,
            limit,
        } => Ok((0, api.tasks(status.as_deref(), before.as_deref(), limit)?)),
        TaskCommand::Show { id } => Ok((0, json!({"task": api.task(&id.to_string())?}))),
        TaskCommand::Cancel { id } => {
            let result = api.cancel_task(&id.to_string())?;
            let exit = i32::from(result["cancellation"]["cancellationConfirmed"] != true);
            Ok((exit, result))
        }
        TaskCommand::Wait {
            id,
            session: session_id,
        } => {
            let proof_session = session::load(session_id)?;
            let logs = proof_session.project_root.join(".intent/logs");
            let id = id.to_string();
            for root in [&logs, &session::workspace(&proof_session)] {
                crate::storage::safe_parent(&proof_session.project_root, root)?;
                let entries = match std::fs::read_dir(root) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                for entry in entries {
                    let entry = entry?;
                    if !entry.file_type()?.is_dir() {
                        continue;
                    }
                    let operation = if root == &logs {
                        let request = entry.path().join("request.json");
                        crate::storage::safe_parent(&proof_session.project_root, &request)?;
                        let Some(manifest) = std::fs::read(&request)
                            .ok()
                            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                        else {
                            continue;
                        };
                        if manifest["session_id"] != proof_session.session_id.to_string() {
                            continue;
                        }
                        manifest["operation"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned()
                    } else {
                        let name = entry.file_name();
                        let Some(operation) =
                            session::legacy_operation(&name.to_string_lossy()).map(str::to_owned)
                        else {
                            continue;
                        };
                        operation
                    };
                    let (used, limit) = match operation.as_str() {
                        "prove" => (proof_session.attempts_used, config.max_proof_attempts),
                        "validate" => (
                            proof_session.validations_used,
                            config.max_validation_attempts,
                        ),
                        "run" => (proof_session.runs_used, config.max_run_attempts),
                        _ => continue,
                    };
                    let evidence = entry.path();
                    crate::storage::safe_parent(
                        &proof_session.project_root,
                        &evidence.join("result.json"),
                    )?;
                    let Some(snapshot) = std::fs::read(evidence.join("result.json"))
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    else {
                        continue;
                    };
                    let accepted = &snapshot["task"];
                    if accepted["id"] != id {
                        continue;
                    }
                    ensure!(
                        accepted["semantics"]["id"] == proof_session.semantics.id
                            && accepted["semantics"]["commit"] == proof_session.semantics.commit,
                        "Saved task semantics identity is missing or mismatched"
                    );
                    let task = api.complete(
                        accepted,
                        config.poll_interval_seconds,
                        evidence.clone(),
                        &proof_session,
                    )?;
                    return finish(&proof_session, task, evidence, session::budget(used, limit));
                }
            }
            bail!(
                "Task {id} has no saved evidence in session {session_id}; use task show to inspect its server state"
            )
        }
    }
}

fn finish(
    proof_session: &session::Session,
    task: Value,
    evidence: std::path::PathBuf,
    budget: Value,
) -> CommandResult {
    let tool_runs = task.get("toolRuns").and_then(Value::as_array);
    let last_run = tool_runs.and_then(|runs| runs.last());
    let exit = last_run
        .and_then(|run| run.get("exitCode"))
        .and_then(Value::as_i64)
        .and_then(|code| i32::try_from(code).ok());
    let timed_out = task["status"] == "timed_out"
        || last_run.and_then(|run| run["outcome"].as_str()) == Some("timeout");
    // The Intent Prover service can reuse a completed result without executing a tool in this task.
    let completed_without_run = tool_runs.is_some_and(Vec::is_empty)
        && task["status"] == "completed"
        && !task["result"].is_null()
        && task["error"].is_null();
    let mut response = json!({"task": task, "sessionId": proof_session.session_id,
        "budget": budget, "evidenceDir": evidence});
    if evidence.parent() == Some(proof_session.project_root.join(".intent/logs").as_path()) {
        let manifest: Value =
            serde_json::from_slice(&std::fs::read(evidence.join("request.json"))?)?;
        if manifest["operation"] == "prove" {
            response["proofDir"] = json!(evidence.join("artifacts"));
        }
    }
    if exit.is_none() {
        response["exitCode"] = Value::Null;
        if !completed_without_run {
            let error = if timed_out {
                "Intent Prover service task timed out."
            } else {
                "Backend did not provide a final tool exit code."
            };
            response["error"] = json!(error);
        }
    }
    eprintln!(
        "Local HTML report: after updating PROOF.md and SCOPE.md, run {} --project <projectRoot> view --output <new-file.html>. projectRoot: {}. Present the HTML first; saved remote job links are inside.",
        channel::settings().cli,
        proof_session.project_root.display()
    );
    Ok((exit.unwrap_or(i32::from(!completed_without_run)), response))
}

fn submission_failure(evidence: &std::path::Path, error: anyhow::Error) -> anyhow::Error {
    if let Err(saved) = crate::api::write_json(
        evidence,
        "result.json",
        &json!({"status":"submission_failed","error":error.to_string()}),
    ) {
        return error.context(format!("Could not save submission failure: {saved}"));
    }
    error
}

fn exhausted(phase: &str, used: u32, limit: u32) -> CommandResult {
    Ok((
        20,
        json!({"status": "exhausted", "phase": phase,
        "budget": session::budget(used, limit),
        "message": "Attempt limit reached; stop and ask the user for help"}),
    ))
}
