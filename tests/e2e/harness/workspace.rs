




use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);


pub struct TempWorkspace {
    pub root: PathBuf,
}

impl TempWorkspace {
    
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = WORKSPACE_COUNTER.fetch_add(1, Ordering::SeqCst);

        let root = std::env::temp_dir().join(format!("ecolog_e2e_{}_{}", timestamp, counter));

        fs::create_dir_all(&root).expect("Failed to create temp workspace");

        
        let env_content = r#"DB_URL=postgres:
API_KEY=secret_key_123
DEBUG=true
PORT=8080
"#;
        let mut env_file = File::create(root.join(".env")).unwrap();
        env_file.write_all(env_content.as_bytes()).unwrap();

        Self { root }
    }

    
    pub fn with_env(env_content: &str) -> Self {
        let workspace = Self::new();
        let mut env_file = File::create(workspace.root.join(".env")).unwrap();
        env_file.write_all(env_content.as_bytes()).unwrap();
        workspace
    }

    
    pub fn create_file(&self, relative_path: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let mut file = File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    
    pub fn file_uri(&self, relative_path: &str) -> String {
        let path = self.root.join(relative_path);
        format!("file:
    }

    
    pub fn create_config(&self, content: &str) {
        self.create_file("ecolog.toml", content);
    }

    
    pub fn create_multi_language_fixtures(&self) {
        
        self.create_file("app.js", "const url = process.env.DB_URL;");

        
        self.create_file("config.ts", "export const apiKey = process.env.API_KEY;");

        
        self.create_file("main.py", "import os\ndb_url = os.environ['DB_URL']");

        
        self.create_file(
            "lib.rs",
            r#"fn main() { std::env::var("PORT").unwrap(); }"#,
        );

        
        self.create_file(
            "main.go",
            r#"package main
import "os"
func main() { os.Getenv("DEBUG") }"#,
        );
    }

    
    pub fn read_file(&self, relative_path: &str) -> Option<String> {
        let path = self.root.join(relative_path);
        fs::read_to_string(path).ok()
    }

    
    pub fn file_exists(&self, relative_path: &str) -> bool {
        self.root.join(relative_path).exists()
    }

    
    pub fn delete_file(&self, relative_path: &str) -> bool {
        let path = self.root.join(relative_path);
        fs::remove_file(path).is_ok()
    }

    
    pub fn append_to_file(&self, relative_path: &str, content: &str) -> bool {
        let path = self.root.join(relative_path);
        if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&path) {
            file.write_all(content.as_bytes()).is_ok()
        } else {
            false
        }
    }
}

impl Default for TempWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_creation() {
        let workspace = TempWorkspace::new();
        assert!(workspace.root.exists());
        assert!(workspace.file_exists(".env"));
    }

    #[test]
    fn test_file_creation() {
        let workspace = TempWorkspace::new();
        workspace.create_file("test.js", "const x = 1;");
        assert!(workspace.file_exists("test.js"));
        assert_eq!(
            workspace.read_file("test.js"),
            Some("const x = 1;".to_string())
        );
    }

    #[test]
    fn test_file_uri() {
        let workspace = TempWorkspace::new();
        let uri = workspace.file_uri("test.js");
        assert!(uri.starts_with("file:
        assert!(uri.ends_with("test.js"));
    }

    #[test]
    fn test_nested_file_creation() {
        let workspace = TempWorkspace::new();
        workspace.create_file("src/lib/utils.ts", "export const x = 1;");
        assert!(workspace.file_exists("src/lib/utils.ts"));
    }

    #[test]
    fn test_cleanup_on_drop() {
        let root_path;
        {
            let workspace = TempWorkspace::new();
            root_path = workspace.root.clone();
            assert!(root_path.exists());
        }
        
        assert!(!root_path.exists());
    }
}
