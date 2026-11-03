use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::path::Path;

use viewer_application::plan::{AssetIndexView, validate_plan};
use viewer_domain::plan::{MAX_PLAN_BYTES, PlanDocument, PlanId, parse_plan};

use super::{AssetStore, PlanRepository};
use crate::plan::reader::compose_payload;

pub const SERVER_NAME: &str = "viewer-plan";

const TOOL_NAMES: [&str; 7] = [
    "assets_import",
    "marks_read",
    "plan_current",
    "plan_list",
    "plan_rules",
    "plan_validate",
    "plan_write",
];

const RULES: &str = r#"{
  "protocolVersion": "viewer.plan/1",
  "structure": {
    "firstLayer": ["plan", "groups", "shots"],
    "secondLayer": ["dimensions[].values[] (fragment + references)", "shots[].references/spec/model"],
    "note": "group 与 shot 共享一个 id 命名空间；dimension value 是共享片段与共享参考图的载体"
  },
  "identity": {
    "planId": "^[A-Za-z0-9][A-Za-z0-9._-]*$ (<=64)",
    "rename": "改 id 必须在 relocations 里声明，否则按删除+新增处理"
  },
  "counts": {
    "declaredCount": "每个 group 必须声明张数；与实际分镜数不一致时返回 count_mismatch 警告"
  },
  "composition": "prompt = group.brief + 各 dimension value.fragment（按 dimensions 顺序）+ shot.delta；shot.prompt 非空时整条覆盖",
  "assets": "参考图必须先经 assets_import 取得 assetId，计划里只写 assetId，不写路径",
  "limits": {
    "groups": 50,
    "shots": 500,
    "dimensions": 20,
    "valuesPerDimension": 50,
    "rules": 50,
    "referencesPerShot": 20,
    "documentBytes": 8388608
  }
}"#;

pub fn serve<R: Read, W: Write>(input: R, mut output: W) {
    let reader = std::io::BufReader::new(input);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        let payload = match dispatch(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message }
            }),
        };
        let _ = writeln!(output, "{payload}");
        let _ = output.flush();
    }
}

fn dispatch(method: &str, params: &Value) -> Result<Value, (i64, String)> {
    match method {
        "initialize" => Ok(initialize(params)),
        "tools/list" => Ok(tools_list()),
        "tools/call" => call_tool(params),
        "ping" => Ok(json!({})),
        other => Err((-32601, format!("unknown method `{other}`"))),
    }
}

fn initialize(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("2024-11-05");
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": "0.3.0" }
    })
}

fn tools_list() -> Value {
    let tools: Vec<Value> = TOOL_NAMES
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "description": description(name),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "projectRoot": { "type": "string" },
                        "planId": { "type": "string" },
                        "plan": { "type": "object" },
                        "path": { "type": "string" }
                    },
                    "required": ["projectRoot"]
                }
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn description(name: &str) -> &'static str {
    match name {
        "plan_rules" => "Return the generation plan authoring rules and limits.",
        "plan_validate" => "Validate a candidate plan and return located errors and warnings.",
        "plan_write" => "Validate and write a new immutable plan revision.",
        "plan_current" => "Return the highest valid revision with composed prompts and hashes.",
        "plan_list" => "List every valid plan in the project.",
        "marks_read" => "Return the user marks recorded by the desktop app.",
        "assets_import" => "Import a reference image and return its content-addressed assetId.",
        _ => "",
    }
}

fn call_tool(params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "tools/call requires a name".to_owned()))?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let outcome = match name {
        "plan_rules" => Ok((json!({ "rules": RULES }), false)),
        "plan_validate" => tool_validate(&arguments),
        "plan_write" => tool_write(&arguments),
        "plan_current" => tool_current(&arguments),
        "plan_list" => tool_list(&arguments),
        "marks_read" => tool_marks(&arguments),
        "assets_import" => tool_import(&arguments),
        other => Err((-32602, format!("unknown tool `{other}`"))),
    };

    match outcome {
        Ok((payload, is_error)) => Ok(json!({
            "content": [{ "type": "text", "text": serde_json::to_string(&payload).unwrap_or_default() }],
            "isError": is_error
        })),
        Err((code, message)) => Err((code, message)),
    }
}

fn project_root(arguments: &Value) -> Result<&Path, (i64, String)> {
    let root = arguments
        .get("projectRoot")
        .and_then(Value::as_str)
        .ok_or((-32602, "projectRoot is required".to_owned()))?;
    Ok(Path::new(root))
}

fn plan_id(arguments: &Value) -> Result<PlanId, (i64, String)> {
    let raw = arguments
        .get("planId")
        .and_then(Value::as_str)
        .ok_or((-32602, "planId is required".to_owned()))?;
    PlanId::parse(raw).map_err(|error| (-32602, error.message))
}

fn parse_document(value: &Value) -> Result<PlanDocument, Value> {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    parse_plan(&bytes, MAX_PLAN_BYTES).map_err(|error| {
        json!({
            "ok": false,
            "errors": [{
                "path": error.path,
                "code": error.code.as_str(),
                "message": error.message,
            }],
            "warnings": [],
        })
    })
}

fn validate_against(root: &Path, document: &PlanDocument) -> Result<Value, (i64, String)> {
    let store = AssetStore::open(root).map_err(|error| (-32603, error.to_string()))?;
    let validation = validate_plan(document, &store as &dyn AssetIndexView);
    Ok(json!({
        "ok": validation.errors.is_empty(),
        "errors": validation.errors,
        "warnings": validation.warnings,
    }))
}

fn tool_validate(arguments: &Value) -> Result<(Value, bool), (i64, String)> {
    let root = project_root(arguments)?;
    let Some(candidate) = arguments.get("plan") else {
        return Err((-32602, "plan is required".to_owned()));
    };
    let document = match parse_document(candidate) {
        Ok(document) => document,
        Err(payload) => return Ok((payload, true)),
    };
    let validation = validate_against(root, &document)?;
    let failed = validation["ok"] == Value::Bool(false);
    Ok((validation, failed))
}

fn tool_write(arguments: &Value) -> Result<(Value, bool), (i64, String)> {
    let root = project_root(arguments)?;
    let Some(candidate) = arguments.get("plan") else {
        return Err((-32602, "plan is required".to_owned()));
    };
    let document = match parse_document(candidate) {
        Ok(document) => document,
        Err(payload) => return Ok((payload, true)),
    };
    let validation = validate_against(root, &document)?;
    if validation["ok"] == Value::Bool(false) {
        return Ok((validation, true));
    }

    let directory = root
        .join(".viewer")
        .join("plans")
        .join("incoming")
        .join(document.plan_id.as_str());
    let path = directory.join(format!("{}.json", document.revision.get()));
    if path.exists() {
        return Ok((
            json!({
                "ok": false,
                "errors": [{
                    "path": "revision",
                    "code": "revision_conflict",
                    "message": format!("revision {} is immutable and already exists", document.revision.get()),
                }],
                "warnings": [],
            }),
            true,
        ));
    }
    std::fs::create_dir_all(&directory).map_err(|error| (-32603, error.to_string()))?;
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| (-32603, error.to_string()))?;
    std::fs::write(&path, bytes).map_err(|error| (-32603, error.to_string()))?;
    Ok((
        json!({ "ok": true, "revision": document.revision.get(), "warnings": validation["warnings"] }),
        false,
    ))
}

fn tool_current(arguments: &Value) -> Result<(Value, bool), (i64, String)> {
    let root = project_root(arguments)?;
    let id = plan_id(arguments)?;
    let repository = PlanRepository::open(root).map_err(|error| (-32603, error.to_string()))?;
    match repository.current(&id) {
        Ok(published) => Ok((compose_payload(&published), false)),
        Err(error) => Ok((json!({ "ok": false, "message": error.to_string() }), true)),
    }
}

fn tool_list(arguments: &Value) -> Result<(Value, bool), (i64, String)> {
    let root = project_root(arguments)?;
    let repository = PlanRepository::open(root).map_err(|error| (-32603, error.to_string()))?;
    let plans: Vec<Value> = repository
        .list()
        .iter()
        .map(|summary| {
            json!({
                "planId": summary.plan_id,
                "revision": summary.revision,
                "ignored": summary.ignored,
            })
        })
        .collect();
    Ok((json!({ "ok": true, "plans": plans }), false))
}

fn tool_marks(arguments: &Value) -> Result<(Value, bool), (i64, String)> {
    let root = project_root(arguments)?;
    let id = plan_id(arguments)?;
    let path = root
        .join(".viewer")
        .join("plan-marks")
        .join(id.as_str())
        .join("current.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|error| (-32603, error.to_string()))?;
            Ok((json!({ "ok": true, "marks": value }), false))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((json!({ "ok": true, "marks": Value::Null }), false))
        }
        Err(error) => Err((-32603, error.to_string())),
    }
}

fn tool_import(arguments: &Value) -> Result<(Value, bool), (i64, String)> {
    let root = project_root(arguments)?;
    let source = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or((-32602, "path is required".to_owned()))?;
    let store = AssetStore::open(root).map_err(|error| (-32603, error.to_string()))?;
    match store.import_path(Path::new(source)) {
        Ok(imported) => Ok((
            json!({
                "ok": true,
                "assetId": imported.asset_id.as_hex(),
                "deduped": imported.deduped,
            }),
            false,
        )),
        Err(error) => Ok((json!({ "ok": false, "message": error.to_string() }), true)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(lines: &[Value]) -> Vec<Value> {
        let input = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut output = Vec::new();
        serve(input.as_bytes(), &mut output);
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn tool_call(id: u32, name: &str, arguments: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        })
    }

    fn payload(response: &Value) -> Value {
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn initialize_advertises_the_tools_capability() {
        let responses = call(&[json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "t" } }
        })]);
        assert_eq!(responses[0]["result"]["serverInfo"]["name"], "viewer-plan");
        assert!(responses[0]["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn tools_list_advertises_the_seven_tools() {
        let responses = call(&[json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })]);
        let mut names: Vec<_> = responses[0]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "assets_import",
                "marks_read",
                "plan_current",
                "plan_list",
                "plan_rules",
                "plan_validate",
                "plan_write",
            ]
        );
    }

    #[test]
    fn plan_validate_returns_located_errors() {
        let directory = tempfile::tempdir().unwrap();
        let plan = json!({
            "protocolVersion": "viewer.plan/1",
            "planId": "autumn",
            "revision": 1,
            "createdAt": "2026-11-03T00:00:00Z",
            "producer": { "name": "harness" },
            "groups": [{ "id": "g1", "label": "夏", "declaredCount": 1, "brief": "b" }],
            "shots": [{
                "id": "s1",
                "groupId": "g1",
                "references": [{ "assetId": "0707070707070707070707070707070707070707070707070707070707070707" }]
            }]
        });
        let responses = call(&[tool_call(
            3,
            "plan_validate",
            json!({ "projectRoot": directory.path(), "plan": plan }),
        )]);
        let body = payload(&responses[0]);
        assert_eq!(body["ok"], false);
        assert_eq!(body["errors"][0]["code"], "asset_missing");
        assert_eq!(body["errors"][0]["path"], "shots[0].references[0]");
    }

    #[test]
    fn plan_write_refuses_invalid_plans_and_writes_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let plan = json!({
            "protocolVersion": "viewer.plan/1",
            "planId": "autumn",
            "revision": 1,
            "createdAt": "2026-11-03T00:00:00Z",
            "producer": { "name": "harness" },
            "groups": [{ "id": "g1", "label": "夏", "declaredCount": 1, "brief": "b" }],
            "shots": [{ "id": "s1", "groupId": "missing" }]
        });
        let responses = call(&[tool_call(
            4,
            "plan_write",
            json!({ "projectRoot": directory.path(), "plan": plan }),
        )]);
        assert_eq!(responses[0]["result"]["isError"], true);
        assert!(!directory.path().join(".viewer/plans/incoming/autumn/1.json").exists());
    }

    #[test]
    fn assets_import_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("hero.png");
        std::fs::write(&source, b"\x89PNG\r\n\x1a\npayload").unwrap();
        let arguments = json!({ "projectRoot": directory.path(), "path": source });
        let responses = call(&[
            tool_call(5, "assets_import", arguments.clone()),
            tool_call(6, "assets_import", arguments),
        ]);
        let first = payload(&responses[0]);
        let second = payload(&responses[1]);
        assert_eq!(first["assetId"], second["assetId"]);
        assert_eq!(first["deduped"], false);
        assert_eq!(second["deduped"], true);
    }
}
