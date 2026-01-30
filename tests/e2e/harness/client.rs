




use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};


static REQUEST_ID: AtomicI64 = AtomicI64::new(1);


pub struct LspTestClient {
    
    _child: Child,
    
    stdin: Arc<Mutex<ChildStdin>>,
    
    pending_responses: Arc<RwLock<HashMap<i64, Value>>>,
    
    notifications: Arc<RwLock<Vec<JsonRpcNotification>>>,
    
    _reader_handle: thread::JoinHandle<()>,
    
    pub workspace_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: i64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl LspTestClient {
    
    pub fn spawn(workspace_root: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        
        let lsp_binary = std::env::var("ECOLOG_LSP_BINARY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                
                let local_binary = manifest_dir.join("target").join("debug").join("ecolog-lsp");
                if local_binary.exists() {
                    return local_binary;
                }
                
                manifest_dir
                    .parent()
                    .map(|p| p.join("target").join("debug").join("ecolog-lsp"))
                    .unwrap_or(local_binary)
            });

        if !lsp_binary.exists() {
            return Err(format!(
                "LSP binary not found at {:?}. Run 'cargo build' first.",
                lsp_binary
            )
            .into());
        }

        let mut child = Command::new(&lsp_binary)
            .current_dir(&workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().expect("Failed to capture stdin");
        let stdout = child.stdout.take().expect("Failed to capture stdout");

        let stdin = Arc::new(Mutex::new(stdin));
        let pending_responses = Arc::new(RwLock::new(HashMap::new()));
        let notifications = Arc::new(RwLock::new(Vec::new()));


        let pending_clone = Arc::clone(&pending_responses);
        let notifications_clone = Arc::clone(&notifications);
        let stdin_clone = Arc::clone(&stdin);

        let reader_handle = thread::spawn(move || {
            Self::read_messages(stdout, pending_clone, notifications_clone, stdin_clone);
        });

        Ok(Self {
            _child: child,
            stdin,
            pending_responses,
            notifications,
            _reader_handle: reader_handle,
            workspace_root,
        })
    }


    fn read_messages(
        stdout: ChildStdout,
        pending: Arc<RwLock<HashMap<i64, Value>>>,
        notifications: Arc<RwLock<Vec<JsonRpcNotification>>>,
        stdin: Arc<Mutex<ChildStdin>>,
    ) {
        let mut reader = BufReader::new(stdout);

        loop {

            let mut content_length: Option<usize> = None;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let line = line.trim();
                if line.is_empty() {
                    break;
                }
                if let Some(len_str) = line.strip_prefix("Content-Length:") {
                    content_length = len_str.trim().parse().ok();
                }
            }

            let Some(len) = content_length else {
                continue;
            };


            let mut content = vec![0u8; len];
            if std::io::Read::read_exact(&mut reader, &mut content).is_err() {
                return;
            }

            let Ok(message): Result<Value, _> = serde_json::from_slice(&content) else {
                continue;
            };


            if let Some(id) = message.get("id").and_then(|v| v.as_i64()) {

                if message.get("result").is_some() || message.get("error").is_some() {
                    pending.write().unwrap().insert(id, message);
                } else if message.get("method").is_some() {
                    // Server-to-client request: respond with empty success
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": null
                    });
                    let content = serde_json::to_string(&response).unwrap();
                    let header = format!("Content-Length: {}\r\n\r\n", content.len());
                    if let Ok(mut stdin_guard) = stdin.lock() {
                        let _ = stdin_guard.write_all(header.as_bytes());
                        let _ = stdin_guard.write_all(content.as_bytes());
                        let _ = stdin_guard.flush();
                    }
                }
            } else if message.get("method").is_some() {

                if let Ok(notif) = serde_json::from_value::<JsonRpcNotification>(message) {
                    notifications.write().unwrap().push(notif);
                }
            }
        }
    }

    
    pub fn request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        self.request_with_timeout(method, params, Duration::from_secs(30))
    }

    
    pub fn request_with_timeout(
        &self,
        method: &str,
        params: Option<Value>,
        duration: Duration,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let id = REQUEST_ID.fetch_add(1, Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        
        self.send_message(&serde_json::to_value(&request)?)?;

        
        let start = Instant::now();
        loop {
            
            {
                let mut responses = self.pending_responses.write().unwrap();
                if let Some(response) = responses.remove(&id) {
                    if let Some(error) = response.get("error") {
                        return Err(format!("LSP Error: {:?}", error).into());
                    }
                    return Ok(response.get("result").cloned().unwrap_or(Value::Null));
                }
            }

            if start.elapsed() > duration {
                return Err(format!("Request '{}' timed out after {:?}", method, duration).into());
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    
    pub fn notify(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.send_message(&notification)
    }

    
    fn send_message(&self, message: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        let mut stdin = self.stdin.lock().unwrap();
        stdin.write_all(header.as_bytes())?;
        stdin.write_all(content.as_bytes())?;
        stdin.flush()?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_notifications(&self) -> Vec<JsonRpcNotification> {
        self.notifications.read().unwrap().clone()
    }

    
    pub fn get_notifications_by_method(&self, method: &str) -> Vec<JsonRpcNotification> {
        self.notifications
            .read()
            .unwrap()
            .iter()
            .filter(|n| n.method == method)
            .cloned()
            .collect()
    }

    
    pub fn clear_notifications(&self) {
        self.notifications.write().unwrap().clear();
    }

    
    pub fn wait_for_notification(
        &self,
        method: &str,
        timeout_duration: Duration,
    ) -> Option<JsonRpcNotification> {
        let start = Instant::now();
        loop {
            let notifications = self.get_notifications_by_method(method);
            if let Some(n) = notifications.last() {
                return Some(n.clone());
            }
            if start.elapsed() > timeout_duration {
                return None;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    
    pub fn initialize(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let init_params = json!({
            "processId": std::process::id(),
            "rootUri": format!("file://{}", self.workspace_root.display()),
            "rootPath": self.workspace_root.display().to_string(),
            "capabilities": {
                "textDocument": {
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "completion": {
                        "completionItem": {
                            "snippetSupport": true,
                            "documentationFormat": ["markdown", "plaintext"]
                        }
                    },
                    "definition": {},
                    "references": {},
                    "rename": { "prepareSupport": true },
                    "publishDiagnostics": {}
                },
                "workspace": {
                    "didChangeWatchedFiles": { "dynamicRegistration": true }
                }
            }
        });

        let result = self.request("initialize", Some(init_params))?;

        
        self.notify("initialized", Some(json!({})))?;

        
        thread::sleep(Duration::from_millis(500));

        Ok(result)
    }

    
    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.request("shutdown", None)?;
        self.notify("exit", None)?;
        Ok(())
    }

    
    pub fn open_document(
        &self,
        uri: &str,
        language_id: &str,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.notify(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            })),
        )
    }

    
    pub fn change_document(
        &self,
        uri: &str,
        version: i32,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.notify(
            "textDocument/didChange",
            Some(json!({
                "textDocument": {
                    "uri": uri,
                    "version": version
                },
                "contentChanges": [{ "text": text }]
            })),
        )
    }

    
    pub fn close_document(&self, uri: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.notify(
            "textDocument/didClose",
            Some(json!({
                "textDocument": { "uri": uri }
            })),
        )
    }

    
    pub fn hover(&self, uri: &str, line: u32, character: u32) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "textDocument/hover",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            })),
        )
    }

    
    pub fn completion(&self, uri: &str, line: u32, character: u32) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "textDocument/completion",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            })),
        )
    }

    
    pub fn definition(&self, uri: &str, line: u32, character: u32) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "textDocument/definition",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            })),
        )
    }

    
    pub fn references(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "textDocument/references",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "context": { "includeDeclaration": include_declaration }
            })),
        )
    }

    
    pub fn prepare_rename(&self, uri: &str, line: u32, character: u32) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "textDocument/prepareRename",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            })),
        )
    }

    
    pub fn rename(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "textDocument/rename",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "newName": new_name
            })),
        )
    }


    pub fn execute_command(
        &self,
        command: &str,
        arguments: Vec<Value>,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "workspace/executeCommand",
            Some(json!({
                "command": command,
                "arguments": arguments
            })),
        )
    }

    pub fn workspace_symbol(&self, query: &str) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "workspace/symbol",
            Some(json!({
                "query": query
            })),
        )
    }

    /// Request inlay hints for a range within a document
    pub fn inlay_hint(
        &self,
        uri: &str,
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        self.request(
            "textDocument/inlayHint",
            Some(json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": start_line, "character": start_character },
                    "end": { "line": end_line, "character": end_character }
                }
            })),
        )
    }
}
