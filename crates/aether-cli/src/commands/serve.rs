use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;

use crate::manifest;

const DEFAULT_PORT: u16 = 3000;

pub fn serve_project(path: &str, port: Option<u16>) -> Result<(), String> {
    let dir = Path::new(path);
    let port = port.unwrap_or(DEFAULT_PORT);

    let m = manifest::load_manifest(dir)?;
    let errors = manifest::validate_manifest(dir, &m);
    if !errors.is_empty() {
        for err in &errors {
            eprintln!("  warning: {err}");
        }
    }

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .map_err(|e| format!("failed to bind to {addr}: {e}"))?;

    println!("Serving '{}' v{}", m.name, m.version);
    println!("  http://{addr}");
    println!("  Press Ctrl+C to stop");

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("connection error: {e}");
                continue;
            }
        };

        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);

        let first_line = request.lines().next().unwrap_or("");
        let req_path = first_line.split_whitespace().nth(1).unwrap_or("/");

        let (status, content_type, body) = route(dir, &m, req_path);

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    }

    Ok(())
}

fn route(dir: &Path, manifest: &manifest::WorldManifest, path: &str) -> (&'static str, &'static str, String) {
    match path {
        "/" => {
            let body = format!(
                r#"{{"name":"{}","version":"{}","description":"{}","scripts":{}}}"#,
                manifest.name,
                manifest.version,
                manifest.description,
                serde_json::array(&manifest.scripts),
            );
            ("200 OK", "application/json", body)
        }
        "/health" => ("200 OK", "application/json", r#"{"status":"ok"}"#.to_string()),
        _ if path.starts_with("/assets/") => {
            let file_path = dir.join(&path[1..]); // strip leading /
            match std::fs::read_to_string(&file_path) {
                Ok(content) => ("200 OK", "application/octet-stream", content),
                Err(_) => ("404 Not Found", "text/plain", "not found".to_string()),
            }
        }
        _ => ("404 Not Found", "text/plain", "not found".to_string()),
    }
}

mod serde_json {
    pub fn array(items: &[String]) -> String {
        let inner: Vec<String> = items.iter().map(|s| format!(r#""{}""#, s)).collect();
        format!("[{}]", inner.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_manifest() -> manifest::WorldManifest {
        manifest::WorldManifest {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            description: "A test world".to_string(),
            scripts: vec!["scripts/main.lua".to_string()],
        }
    }

    #[test]
    fn test_route_root() {
        let tmp = TempDir::new().unwrap();
        let m = test_manifest();
        let (status, ct, body) = route(tmp.path(), &m, "/");
        assert_eq!(status, "200 OK");
        assert_eq!(ct, "application/json");
        assert!(body.contains(r#""name":"test""#));
        assert!(body.contains(r#""version":"0.1.0""#));
    }

    #[test]
    fn test_route_health() {
        let tmp = TempDir::new().unwrap();
        let m = test_manifest();
        let (status, _, body) = route(tmp.path(), &m, "/health");
        assert_eq!(status, "200 OK");
        assert!(body.contains("ok"));
    }

    #[test]
    fn test_route_asset_found() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("assets")).unwrap();
        fs::write(tmp.path().join("assets/test.txt"), "hello").unwrap();
        let m = test_manifest();
        let (status, _, body) = route(tmp.path(), &m, "/assets/test.txt");
        assert_eq!(status, "200 OK");
        assert_eq!(body, "hello");
    }

    #[test]
    fn test_route_asset_not_found() {
        let tmp = TempDir::new().unwrap();
        let m = test_manifest();
        let (status, _, _) = route(tmp.path(), &m, "/assets/nope.txt");
        assert_eq!(status, "404 Not Found");
    }

    #[test]
    fn test_route_unknown() {
        let tmp = TempDir::new().unwrap();
        let m = test_manifest();
        let (status, _, _) = route(tmp.path(), &m, "/unknown");
        assert_eq!(status, "404 Not Found");
    }

    #[test]
    fn test_serve_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        let result = serve_project(tmp.path().to_str().unwrap(), Some(0));
        assert!(result.is_err());
    }

    #[test]
    fn test_serde_json_array() {
        let items = vec!["a.lua".to_string(), "b.lua".to_string()];
        assert_eq!(serde_json::array(&items), r#"["a.lua","b.lua"]"#);
    }

    #[test]
    fn test_serde_json_array_empty() {
        let items: Vec<String> = vec![];
        assert_eq!(serde_json::array(&items), "[]");
    }
}
