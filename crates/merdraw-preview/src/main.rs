use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const HOST: &str = "127.0.0.1:7878";
const MAX_IMAGES: usize = 32;

#[derive(Debug)]
struct AppState {
    images: HashMap<String, PathBuf>,
    order: VecDeque<String>,
    counter: u64,
    rng: u64,
    workspace_root: PathBuf,
}

fn main() {
    let workspace_root = workspace_root();
    let state = Arc::new(Mutex::new(AppState {
        images: HashMap::new(),
        order: VecDeque::new(),
        counter: 0,
        rng: seed_from_time(),
        workspace_root,
    }));

    let listener = TcpListener::bind(HOST).expect("failed to bind preview server");
    eprintln!("preview server running on http://{HOST}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || handle_connection(stream, state));
            }
            Err(err) => eprintln!("connection failed: {err}"),
        }
    }
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(&manifest_dir)
        .to_path_buf()
}

fn handle_connection(mut stream: TcpStream, state: Arc<Mutex<AppState>>) {
    let mut buffer = [0u8; 8192];
    let read = match stream.read(&mut buffer) {
        Ok(0) | Err(_) => return,
        Ok(n) => n,
    };
    let request = String::from_utf8_lossy(&buffer[..read]);
    let (method, path) = match parse_request_line(&request) {
        Some(values) => values,
        None => {
            let _ = respond_text(&mut stream, 400, "Bad Request");
            return;
        }
    };

    if method != "GET" {
        let _ = respond_text(&mut stream, 405, "Method Not Allowed");
        return;
    }

    if path.starts_with("/image") {
        let id = query_param(&path, "id").unwrap_or_default();
        let guard = state.lock().unwrap();
        if let Some(image_path) = guard.images.get(&id) {
            if let Ok(bytes) = fs::read(image_path) {
                let _ = respond_bytes(&mut stream, 200, "image/png", &bytes);
                return;
            }
        }
        let _ = respond_text(&mut stream, 404, "Not Found");
        return;
    }

    if path == "/" || path.starts_with("/next") {
        let mut guard = state.lock().unwrap();
        match render_random(&mut guard) {
            Ok(Rendered {
                id,
                label,
                source,
            }) => {
                let body = render_page(&id, &label, &source);
                let _ = respond_html(&mut stream, 200, &body);
            }
            Err(err) => {
                let body = format!(
                    "<html><body><h1>Render failed</h1><pre>{}</pre></body></html>",
                    escape_html(&err)
                );
                let _ = respond_html(&mut stream, 500, &body);
            }
        }
        return;
    }

    let _ = respond_text(&mut stream, 404, "Not Found");
}

struct Rendered {
    id: String,
    label: String,
    source: String,
}

fn render_random(state: &mut AppState) -> Result<Rendered, String> {
    let (flowchart, label) = generate_flowchart(state);

    let id = next_id(state);
    let output_dir = state.workspace_root.join("tmp");
    let _ = fs::create_dir_all(&output_dir);
    let output_path = output_dir.join(format!("preview_{id}.png"));

    let mut child = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "merdraw",
            "--",
            "--out",
            output_path.to_string_lossy().as_ref(),
        ])
        .current_dir(&state.workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to invoke merdraw: {err}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(flowchart.as_bytes())
            .map_err(|err| format!("failed to write to merdraw stdin: {err}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to wait for merdraw: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("merdraw failed: {stderr}"));
    }

    state.images.insert(id.clone(), output_path);
    state.order.push_back(id.clone());
    while state.order.len() > MAX_IMAGES {
        if let Some(old) = state.order.pop_front() {
            if let Some(path) = state.images.remove(&old) {
                let _ = fs::remove_file(path);
            }
        }
    }

    Ok(Rendered {
        id,
        label,
        source: flowchart,
    })
}

fn next_id(state: &mut AppState) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    state.counter += 1;
    format!("{now}_{}", state.counter)
}

fn seed_from_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn rand_u64(state: &mut AppState) -> u64 {
    state.rng = state
        .rng
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    state.rng
}

fn rand_range(state: &mut AppState, min: usize, max: usize) -> usize {
    if min >= max {
        return min;
    }
    let span = max - min + 1;
    min + (rand_u64(state) as usize % span)
}

fn node_id(index: usize) -> String {
    if index < 26 {
        let ch = (b'A' + index as u8) as char;
        return ch.to_string();
    }
    let first = ((index / 26) - 1) as u8;
    let second = (index % 26) as u8;
    let a = (b'A' + first) as char;
    let b = (b'A' + second) as char;
    format!("{a}{b}")
}

fn generate_flowchart(state: &mut AppState) -> (String, String) {
    let node_count = rand_range(state, 4, 10);
    let mut nodes = Vec::new();
    for i in 0..node_count {
        let id = node_id(i);
        let label = format!("Node {}", i + 1);
        nodes.push((id, label));
    }

    let mut edges = HashMap::new();
    for to in 1..node_count {
        let incoming = rand_range(state, 1, 2);
        for _ in 0..incoming {
            let from = rand_range(state, 0, to - 1);
            edges.insert((from, to), random_label(state));
        }
    }

    let extra = rand_range(state, 0, node_count);
    for _ in 0..extra {
        let from = rand_range(state, 0, node_count - 2);
        let to = rand_range(state, from + 1, node_count - 1);
        edges.insert((from, to), random_label(state));
    }

    let mut output = String::from("flowchart TB\n");
    for (id, label) in &nodes {
        output.push_str(&format!("{id}[{label}]\n"));
    }

    for ((from, to), label) in edges {
        let from_id = &nodes[from].0;
        let to_id = &nodes[to].0;
        if let Some(label) = label {
            output.push_str(&format!("{from_id} -->|{label}| {to_id}\n"));
        } else {
            output.push_str(&format!("{from_id} --> {to_id}\n"));
        }
    }

    let label = format!("Random flowchart ({node_count} nodes)");
    (output, label)
}

fn random_label(state: &mut AppState) -> Option<&'static str> {
    let labels = [
        "request",
        "response",
        "cache?",
        "miss",
        "hit",
        "retry",
        "ok",
        "fail",
        "event",
    ];
    if rand_range(state, 0, 3) == 0 {
        let idx = rand_range(state, 0, labels.len() - 1);
        Some(labels[idx])
    } else {
        None
    }
}

fn render_page(image_id: &str, label: &str, source: &str) -> String {
    let source = escape_html(source);
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>merdraw preview</title>
  <style>
    body {{ font-family: ui-sans-serif, system-ui, sans-serif; margin: 24px; }}
    .toolbar {{ display: flex; gap: 12px; align-items: center; margin-bottom: 16px; }}
    .preview img {{ max-width: 100%; height: auto; border: 1px solid #ccc; }}
    pre {{ background: #f6f6f6; padding: 12px; border: 1px solid #ddd; overflow-x: auto; }}
    button {{ padding: 8px 14px; font-size: 14px; }}
  </style>
</head>
<body>
  <div class="toolbar">
    <form action="/next" method="get">
      <button type="submit">Next</button>
    </form>
    <div>Example: {label}</div>
  </div>
  <div class="preview">
    <img src="/image?id={image_id}" alt="flowchart preview" />
  </div>
  <pre>{source}</pre>
</body>
</html>"#
    )
}

fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let mut lines = request.lines();
    let line = lines.next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

fn query_param(path: &str, key: &str) -> Option<String> {
    let mut split = path.splitn(2, '?');
    split.next()?;
    let query = split.next()?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let Some(k) = kv.next() {
            if k == key {
                return kv.next().map(|v| v.to_string());
            }
        }
    }
    None
}

fn respond_html(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    respond_bytes(stream, status, "text/html; charset=utf-8", body.as_bytes())
}

fn respond_text(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    respond_bytes(stream, status, "text/plain; charset=utf-8", body.as_bytes())
}

fn respond_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let status_line = match status {
        200 => "HTTP/1.1 200 OK",
        400 => "HTTP/1.1 400 Bad Request",
        404 => "HTTP/1.1 404 Not Found",
        405 => "HTTP/1.1 405 Method Not Allowed",
        500 => "HTTP/1.1 500 Internal Server Error",
        _ => "HTTP/1.1 500 Internal Server Error",
    };
    let headers = format!(
        "{status_line}\r\nContent-Length: {}\r\nContent-Type: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len(),
        content_type
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
