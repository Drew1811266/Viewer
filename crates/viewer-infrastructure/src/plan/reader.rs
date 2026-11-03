use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};

use super::{PlanRepository, PublishedPlan, RepoError};
use viewer_application::plan::compose_plan;
use viewer_domain::plan::PlanId;

pub const READER_PROTOCOL_V1: &str = "viewer.plan.reader/1";
const MAX_REQUEST_BYTES: u64 = 64 * 1024;

#[derive(Deserialize)]
struct ReaderRequest {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    operation: String,
    #[serde(rename = "projectRoot")]
    project_root: Option<String>,
    #[serde(rename = "planId")]
    plan_id: Option<String>,
}

pub fn run_plan_reader<R: Read, W: Write>(input: R, mut output: W) -> bool {
    let mut buffer = Vec::new();
    if input
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut buffer)
        .is_err()
        || buffer.len() as u64 > MAX_REQUEST_BYTES
    {
        return fail(&mut output, "io", "reader request could not be read");
    }
    let request: ReaderRequest = match serde_json::from_slice(&buffer) {
        Ok(request) => request,
        Err(error) => return fail(&mut output, "integrity", &error.to_string()),
    };
    if request.protocol_version != READER_PROTOCOL_V1 {
        return fail(
            &mut output,
            "unsupported_version",
            "reader protocol version is not supported",
        );
    }
    let Some(project_root) = request.project_root.as_deref() else {
        return fail(&mut output, "integrity", "--project is required");
    };

    let repository = match PlanRepository::open(std::path::Path::new(project_root)) {
        Ok(repository) => repository,
        Err(error) => return fail(&mut output, "io", &error.to_string()),
    };

    let result = match request.operation.as_str() {
        "current" => current(&repository, request.plan_id.as_deref()),
        "list" => Ok(list(&repository)),
        "marks" => marks(project_root, request.plan_id.as_deref()),
        other => Err(("unknown_operation".to_owned(), format!("unknown operation `{other}`"))),
    };

    match result {
        Ok(payload) => succeed(&mut output, payload),
        Err((code, message)) => fail(&mut output, &code, &message),
    }
}

pub(crate) fn compose_payload(published: &PublishedPlan) -> Value {
    let composed = compose_plan(&published.plan);
    let shots: Vec<Value> = composed
        .iter()
        .map(|shot| {
            json!({
                "shotId": shot.shot_id,
                "prompt": shot.prompt,
                "hash": shot.hash,
                "references": shot.references.iter().map(|r| r.asset_id.as_hex()).collect::<Vec<_>>(),
            })
        })
        .collect();

    json!({
        "status": "ok",
        "planId": published.plan.plan_id.as_str(),
        "revision": published.plan.revision.get(),
        "blake3": published.blake3,
        "ignored": published.ignored.iter().map(|entry| json!({
            "revision": entry.revision,
            "code": entry.code,
            "message": entry.message,
        })).collect::<Vec<_>>(),
        "shots": shots,
    })
}

fn current(repository: &PlanRepository, plan_id: Option<&str>) -> Result<Value, (String, String)> {
    let plan_id = plan_id.ok_or_else(|| ("integrity".to_owned(), "planId is required".to_owned()))?;
    let parsed = PlanId::parse(plan_id).map_err(|error| (error.code.as_str().to_owned(), error.message))?;
    let published = repository.current(&parsed).map_err(repo_error)?;
    let mut payload = compose_payload(&published);
    payload["status"] = Value::String("ok".to_owned());
    Ok(payload)
}

fn list(repository: &PlanRepository) -> Value {
    json!({
        "status": "ok",
        "plans": repository.list().iter().map(|summary| json!({
            "planId": summary.plan_id,
            "revision": summary.revision,
            "ignored": summary.ignored,
        })).collect::<Vec<_>>(),
    })
}

fn marks(project_root: &str, plan_id: Option<&str>) -> Result<Value, (String, String)> {
    let plan_id = plan_id.ok_or_else(|| ("integrity".to_owned(), "planId is required".to_owned()))?;
    let path = std::path::Path::new(project_root)
        .join(".viewer")
        .join("plan-marks")
        .join(plan_id)
        .join("current.json");
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
            .map(|value| json!({ "status": "ok", "marks": value }))
            .map_err(|error| ("integrity".to_owned(), error.to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(json!({ "status": "ok", "marks": Value::Null }))
        }
        Err(error) => Err(("io".to_owned(), error.to_string())),
    }
}

fn repo_error(error: RepoError) -> (String, String) {
    let code = match &error {
        RepoError::NoPlan => "no_plan",
        RepoError::UnknownPlan(_) => "unknown_plan",
        RepoError::InvalidPlan(_) => "invalid_plan",
        RepoError::Io(_) => "io",
    };
    (code.to_owned(), error.to_string())
}

fn succeed<W: Write>(output: &mut W, payload: Value) -> bool {
    write_line(output, &payload)
}

fn fail<W: Write>(output: &mut W, code: &str, message: &str) -> bool {
    write_line(output, &json!({ "status": "error", "code": code, "message": message }));
    false
}

fn write_line<W: Write>(output: &mut W, payload: &Value) -> bool {
    match serde_json::to_string(payload) {
        Ok(line) => output.write_all(line.as_bytes()).is_ok() && output.write_all(b"\n").is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with_plan() -> tempfile::TempDir {
        use viewer_domain::plan::{PlanDocument, PlanGroup, PlanShot, Producer, Revision};
        use std::collections::BTreeMap;

        let directory = tempfile::tempdir().unwrap();
        let document = PlanDocument {
            protocol_version: "viewer.plan/1".to_owned(),
            plan_id: PlanId::parse("autumn").unwrap(),
            revision: Revision::new(1),
            title: None,
            created_at: "2026-11-03T00:00:00Z".to_owned(),
            producer: Producer {
                name: "harness".to_owned(),
                session_id: None,
            },
            dimensions: Vec::new(),
            rules: Vec::new(),
            groups: vec![PlanGroup {
                id: "g1".to_owned(),
                label: "夏".to_owned(),
                attributes: BTreeMap::new(),
                declared_count: 1,
                brief: "轻薄夏装".to_owned(),
            }],
            shots: vec![PlanShot {
                id: "s1".to_owned(),
                group_id: "g1".to_owned(),
                attributes: BTreeMap::new(),
                delta: Some("海边逆光".to_owned()),
                prompt: None,
                references: Vec::new(),
                spec: None,
                model: None,
            }],
            relocations: Vec::new(),
        };
        let path = directory
            .path()
            .join(".viewer/plans/incoming/autumn/1.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&document).unwrap()).unwrap();
        directory
    }

    fn run(request: &str) -> (bool, Value) {
        let mut output = Vec::new();
        let ok = run_plan_reader(request.as_bytes(), &mut output);
        let value: Value = serde_json::from_slice(&output).unwrap();
        (ok, value)
    }

    #[test]
    fn current_returns_composed_prompts_and_hashes() {
        let directory = project_with_plan();
        let request = format!(
            r#"{{"protocolVersion":"viewer.plan.reader/1","operation":"current","projectRoot":"{}","planId":"autumn"}}"#,
            directory.path().display()
        );
        let (ok, response) = run(&request);
        assert!(ok, "{response}");
        assert_eq!(response["status"], "ok");
        assert_eq!(response["revision"], 1);
        assert_eq!(response["shots"][0]["prompt"], "轻薄夏装\n海边逆光");
        assert!(response["shots"][0]["hash"].is_string());
    }

    #[test]
    fn unsupported_version_does_not_report_corruption() {
        let (ok, response) = run(r#"{"protocolVersion":"viewer.plan.reader/9","operation":"list"}"#);
        assert!(!ok);
        assert_eq!(response["code"], "unsupported_version");
    }

    #[test]
    fn unknown_operation_is_reported() {
        let directory = tempfile::tempdir().unwrap();
        let request = format!(
            r#"{{"protocolVersion":"viewer.plan.reader/1","operation":"nope","projectRoot":"{}"}}"#,
            directory.path().display()
        );
        let (ok, response) = run(&request);
        assert!(!ok);
        assert_eq!(response["code"], "unknown_operation");
    }

    #[test]
    fn marks_returns_null_when_the_project_has_none() {
        let directory = tempfile::tempdir().unwrap();
        let request = format!(
            r#"{{"protocolVersion":"viewer.plan.reader/1","operation":"marks","projectRoot":"{}","planId":"autumn"}}"#,
            directory.path().display()
        );
        let (ok, response) = run(&request);
        assert!(ok);
        assert!(response["marks"].is_null());
    }
}
