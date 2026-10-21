//! `projectile --mcp`: a small Model Context Protocol server on stdio, so Claude Code (or any MCP
//! client) can inspect the source, preview a plan and generate a project, exactly like the window.
//!
//! Register it with `claude mcp add projectile -- /path/to/projectile --mcp` (About shows the exact
//! line). The PhotoCraft source comes from the `source` argument, `$PROJECTILE_SOURCE`, or detection.

use crate::generator;
use crate::plan::{self, Options, Profile, ProjectSpec};
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;

const MAX_LINE: u64 = 1 << 20;
const PROTOCOL_VERSION: &str = "2025-06-18";

pub fn serve() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = std::io::stdout();
    loop {
        let mut line = String::new();
        let read = reader.by_ref().take(MAX_LINE).read_line(&mut line).map_err(|error| format!("stdin: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        if !line.ends_with('\n') && read as u64 >= MAX_LINE {
            discard_line(&mut reader)?;
            write_message(&mut stdout, &error_response(Value::Null, -32600, "request line is too long"))?;
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_line(&line) {
            write_message(&mut stdout, &response)?;
        }
    }
}

fn discard_line(reader: &mut impl BufRead) -> Result<(), String> {
    let mut sink = Vec::new();
    reader.read_until(b'\n', &mut sink).map_err(|error| format!("stdin: {error}"))?;
    Ok(())
}

fn write_message(out: &mut impl Write, message: &Value) -> Result<(), String> {
    let text = serde_json::to_string(message).map_err(|error| error.to_string())?;
    writeln!(out, "{text}").and_then(|()| out.flush()).map_err(|error| format!("stdout: {error}"))
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

pub fn handle_line(line: &str) -> Option<Value> {
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => return Some(error_response(Value::Null, -32700, &format!("parse error: {error}"))),
    };
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    // Notifications (no id) never get a response.
    let id = id?;
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "projectile", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "ProjectIle generates PhotoCore projects from the PhotoCraft source. Call projectile_plan first and show the user the summary, then projectile_generate."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => Ok(call_tool(&params)),
        _ => Err((-32601, format!("method `{method}` not found"))),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err((code, message)) => error_response(id, code, &message),
    })
}

fn spec_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "source": {"type": "string", "description": "Absolute path of the PhotoCraft checkout to read (optional; detected when omitted)"},
            "name": {"type": "string", "description": "Project name: an ASCII letter, then letters, digits, - or _"},
            "destination": {"type": "string", "description": "Absolute parent folder; the project is created inside it"},
            "profile": {"type": "string", "enum": ["Base", "Viewer", "Learning demos", "Full", "Minimal"], "description": "Base, Viewer, and Learning demos generate today; Viewer needs options.viewer_image"},
            "options": {
                "type": "object",
                "properties": {
                    "width": {"type": "integer"}, "height": {"type": "integer"}, "scale_percent": {"type": "integer"},
                    "include_docs": {"type": "boolean"}, "backup_source": {"type": "boolean", "description": "Include untouched source backup; Viewer always includes this even when false"}, "backup_projectile": {"type": "boolean", "description": "Include the recoverable ProjectIle source archive that generated the project"}, "vscode_config": {"type": "boolean"},
                    "mcp": {"type": "boolean"}, "mcp_port": {"type": "integer"}, "git_init": {"type": "boolean"},
                    "toolchain": {"type": "string"}, "viewer_image": {"type": "string"},
                    "android": {"type": "boolean"}, "android_min_sdk": {"type": "integer"}, "android_target_sdk": {"type": "integer"},
                    "android_java_version": {"type": "integer", "enum": [17, 21, 25]},
                    "android_gradle_version": {"type": "string", "enum": ["9.4.1", "9.6.0", "9.8.1"]},
                    "show_menu_bar": {"type": "boolean"},
                    "hidden_menus": {"type": "array", "items": {"type": "string", "enum": ["File", "Edit", "Image", "Layer", "Type", "Select", "Filter", "View", "Window", "Help"]}},
                    "hidden_submenus": {"type": "array", "items": {"type": "string", "description": "Full path such as File > Export"}},
                    "visual_debug_menu": {"type": "boolean"},
                    "on_existing": {"type": "string", "enum": ["Stop", "RenameAside", "Merge"], "description": "When the project folder exists: Stop (default), RenameAside (old folder kept as <name>-old-<date>), or Merge (overwrite same-named files, keep others). Deleting is only offered in the window."}
                }
            }
        },
        "required": ["name", "destination"]
    })
}

fn tools() -> Value {
    json!([
        {"name": "projectile_status", "description": "PhotoCraft source location, revision and size, plus the available profiles.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "projectile_plan", "description": "Preview a generation without writing anything: components with +/- status, capability changes, file edits and blockers.", "inputSchema": spec_schema()},
        {"name": "projectile_generate", "description": "Generate the project. Never overwrites an existing folder unless options.on_existing says how.", "inputSchema": spec_schema()},
        {"name": "projectile_regenerate", "description": "Rebuild a project ProjectIle made, from the current source, with the profile and options saved in its .projectile.json (backup refreshed).", "inputSchema": {
            "type": "object",
            "properties": {
                "project": {"type": "string", "description": "Absolute path of the generated project folder"},
                "on_existing": {"type": "string", "enum": ["RenameAside", "Merge"], "description": "RenameAside (default): keep the old folder as <name>-old-<date>. Merge: overwrite same-named files, keep others. Deleting is only offered in the window."},
                "source": {"type": "string", "description": "Absolute path of the PhotoCraft checkout (optional; detected when omitted)"}
            },
            "required": ["project"]
        }}
    ])
}

fn text_result(text: String, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

fn parse_spec(arguments: &Value) -> Result<ProjectSpec, String> {
    let name = arguments.get("name").and_then(Value::as_str).ok_or("`name` (string) is required")?;
    let destination = arguments.get("destination").and_then(Value::as_str).ok_or("`destination` (string) is required")?;
    let destination = PathBuf::from(destination);
    if !destination.is_absolute() {
        return Err("`destination` must be an absolute path".into());
    }
    let profile = match arguments.get("profile").and_then(Value::as_str) {
        None => Profile::Base,
        Some(name) => Profile::ALL.into_iter().find(|p| p.label().eq_ignore_ascii_case(name)).ok_or_else(|| {
            let names: Vec<&str> = Profile::ALL.iter().map(|p| p.label()).collect();
            format!("unknown profile `{name}`; use {}", names.join(", "))
        })?,
    };
    let options = match arguments.get("options") {
        None | Some(Value::Null) => Options::default(),
        Some(value) => serde_json::from_value(value.clone()).map_err(|error| format!("invalid `options`: {error}"))?,
    };
    if options.on_existing == crate::plan::OnExisting::Delete {
        return Err("`on_existing: Delete` is only available in the ProjectIle window, where the user confirms it; use RenameAside or Merge.".into());
    }
    Ok(ProjectSpec { source: resolve_source(arguments)?, name: name.to_owned(), destination, profile, options })
}

fn regenerate_spec_from(arguments: &Value) -> Result<ProjectSpec, String> {
    let project = arguments.get("project").and_then(Value::as_str).ok_or("`project` (string) is required")?;
    let project = PathBuf::from(project);
    if !project.is_absolute() {
        return Err("`project` must be an absolute path".into());
    }
    let mode = match arguments.get("on_existing").and_then(Value::as_str) {
        None | Some("RenameAside") => crate::plan::OnExisting::RenameAside,
        Some("Merge") => crate::plan::OnExisting::Merge,
        Some("Delete") => return Err("Deleting is only available in the ProjectIle window, where the user confirms it.".into()),
        Some(other) => return Err(format!("unknown on_existing `{other}`; use RenameAside or Merge")),
    };
    plan::regenerate_spec(&project, resolve_source(arguments)?, mode)
}

/// The `source` argument when given (it must be valid), otherwise detection.
fn resolve_source(arguments: &Value) -> Result<PathBuf, String> {
    match arguments.get("source") {
        None | Some(Value::Null) => crate::source::detect("")
            .ok_or_else(|| format!("no PhotoCraft source found; pass `source` (absolute path of a PhotoCraft checkout) or set {}", crate::source::SOURCE_ENV)),
        Some(Value::String(path)) => crate::source::validate(&PathBuf::from(path)),
        Some(_) => Err("`source` must be a string path".into()),
    }
}

fn call_tool(params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let inventory = match resolve_source(&arguments).and_then(|root| plan::scan(&root)) {
        Ok(inventory) => inventory,
        Err(error) => return text_result(error, true),
    };
    match name {
        "projectile_status" => {
            let (files, bytes) = inventory.totals();
            let profiles: Vec<String> =
                Profile::ALL.iter().map(|p| format!("{} ({}): {}", p.label(), if p.available() { "ready" } else { "preview" }, p.tagline())).collect();
            text_result(
                format!(
                    "ProjectIle {} (standalone)\nSource: {}\nPhotoCraft version: {}\nRevision: {}\nSize: {files} files, {}\nProfiles:\n- {}",
                    env!("CARGO_PKG_VERSION"),
                    inventory.root.display(),
                    crate::source::photocraft_version(&inventory.root).as_deref().unwrap_or("unknown"),
                    inventory.revision.as_deref().unwrap_or("unknown"),
                    plan::human_bytes(bytes),
                    profiles.join("\n- ")
                ),
                false,
            )
        }
        "projectile_plan" => match parse_spec(&arguments) {
            Ok(spec) => {
                let plan = plan::build(&spec, &inventory);
                let mut text = format!("Project folder: {}\n\nComponents:\n", plan.project_dir.display());
                for c in plan.summary() {
                    text.push_str(&format!("{} {}\n", c.sign, c.text));
                }
                text.push_str("\nCapabilities compared with full PhotoCraft:\n");
                for c in &plan.capabilities {
                    text.push_str(&format!("{} {}\n", c.sign, c.text));
                }
                text.push_str("\nFile edits:\n");
                for edit in &plan.edits {
                    text.push_str(&format!("~ {}: {}\n", edit.path, edit.why));
                }
                for warning in &plan.warnings {
                    text.push_str(&format!("\nWarning: {warning}"));
                }
                for blocker in &plan.blockers {
                    text.push_str(&format!("\nBlocked: {blocker}"));
                }
                text_result(text, false)
            }
            Err(error) => text_result(error, true),
        },
        "projectile_generate" => match parse_spec(&arguments) {
            Ok(spec) => {
                let mut log = Vec::new();
                match generator::generate(&spec, &mut |event| {
                    if let generator::Event::Log(line) = event {
                        log.push(line);
                    }
                }) {
                    Ok(report) => text_result(
                        format!(
                            "Generated {} ({} files).\n{}\nNext: cd into it and run `cargo run -p photocraft`.",
                            report.path.display(),
                            report.files,
                            log.join("\n")
                        ),
                        false,
                    ),
                    Err(error) => text_result(format!("Generation failed: {error}\n{}", log.join("\n")), true),
                }
            }
            Err(error) => text_result(error, true),
        },
        "projectile_regenerate" => match regenerate_spec_from(&arguments) {
            Ok(spec) => {
                let mut log = Vec::new();
                match generator::generate(&spec, &mut |event| {
                    if let generator::Event::Log(line) = event {
                        log.push(line);
                    }
                }) {
                    Ok(report) => text_result(format!("Regenerated {} ({} files).\n{}", report.path.display(), report.files, log.join("\n")), false),
                    Err(error) => text_result(format!("Regeneration failed: {error}\n{}", log.join("\n")), true),
                }
            }
            Err(error) => text_result(error, true),
        },
        other => text_result(format!("unknown tool `{other}`"), true),
    }
}

#[cfg(test)]
mod tests {
    use super::handle_line;

    #[test]
    fn initialize_list_and_notifications() {
        let init = handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        let list = handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        assert_eq!(list["result"]["tools"].as_array().map(Vec::len), Some(4));
        let tools = list["result"]["tools"].as_array().expect("tools list is an array");
        let generate = tools.iter().find(|tool| tool["name"] == "projectile_generate").expect("generate tool exists");
        assert_eq!(generate["inputSchema"]["properties"]["options"]["properties"]["android_java_version"]["enum"][1], 21);
        assert_eq!(generate["inputSchema"]["properties"]["options"]["properties"]["android_gradle_version"]["enum"][2], "9.8.1");
        assert_eq!(generate["inputSchema"]["properties"]["options"]["properties"]["visual_debug_menu"]["type"], "boolean");
        assert_eq!(generate["inputSchema"]["properties"]["options"]["properties"]["hidden_submenus"]["type"], "array");
    }

    #[test]
    fn hostile_requests_are_errors_not_panics() {
        assert_eq!(handle_line("not json").unwrap()["error"]["code"], -32700);
        assert_eq!(handle_line(r#"{"id":3,"method":"nope"}"#).unwrap()["error"]["code"], -32601);
        let bad = handle_line(r#"{"id":4,"method":"tools/call","params":{"name":"projectile_plan","arguments":{"name":5}}}"#).unwrap();
        assert_eq!(bad["result"]["isError"], true);
        let relative =
            handle_line(r#"{"id":5,"method":"tools/call","params":{"name":"projectile_generate","arguments":{"name":"x","destination":"rel"}}}"#).unwrap();
        assert_eq!(relative["result"]["isError"], true);
        let delete = handle_line(r#"{"id":7,"method":"tools/call","params":{"name":"projectile_generate","arguments":{"name":"x","destination":"/tmp","options":{"on_existing":"Delete"}}}}"#).unwrap();
        assert_eq!(delete["result"]["isError"], true, "agents may never delete folders");
        let options = handle_line(
            r#"{"id":6,"method":"tools/call","params":{"name":"projectile_plan","arguments":{"name":"x","destination":"/tmp","options":{"width":"wide"}}}}"#,
        )
        .unwrap();
        assert_eq!(options["result"]["isError"], true);
    }
}
