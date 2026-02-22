use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

struct DaemonHarness {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
}

impl DaemonHarness {
    fn start() -> Self {
        let bin = env!("CARGO_BIN_EXE_openjaxd");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env("OPENJAX_APPROVAL_POLICY", "never")
            .spawn()
            .expect("failed to start openjaxd");

        let stdin = child.stdin.take().expect("failed to open stdin");
        let stdout = child.stdout.take().expect("failed to open stdout");

        let (tx, rx) = mpsc::channel::<Value>();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    let _ = tx.send(value);
                }
            }
        });

        Self { child, stdin, rx }
    }

    fn send(&mut self, message: Value) {
        let payload = serde_json::to_string(&message).expect("failed to serialize request");
        self.stdin
            .write_all(payload.as_bytes())
            .expect("failed to write request");
        self.stdin
            .write_all(b"\n")
            .expect("failed to write newline");
        self.stdin.flush().expect("failed to flush request");
    }

    fn recv_until<F>(&self, timeout: Duration, mut predicate: F) -> Value
    where
        F: FnMut(&Value) -> bool,
    {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            assert!(now < deadline, "timed out waiting for message");
            let remaining = deadline.saturating_duration_since(now);
            let value = self
                .rx
                .recv_timeout(remaining)
                .expect("channel closed while waiting for message");
            if predicate(&value) {
                return value;
            }
        }
    }
}

impl Drop for DaemonHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn request(request_id: &str, session_id: Option<&str>, method: &str, params: Value) -> Value {
    let mut v = json!({
        "protocol_version": "v1",
        "kind": "request",
        "request_id": request_id,
        "method": method,
        "params": params
    });
    if let Some(sid) = session_id {
        v["session_id"] = Value::String(sid.to_string());
    }
    v
}

#[test]
fn m1_protocol_flow_start_stream_submit_shutdown() {
    let mut daemon = DaemonHarness::start();

    daemon.send(request("req-start", None, "start_session", json!({})));
    let start_resp = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-start".to_string()))
    });

    let session_id = start_resp["result"]["session_id"]
        .as_str()
        .expect("session_id missing")
        .to_string();

    daemon.send(request(
        "req-stream",
        Some(&session_id),
        "stream_events",
        json!({}),
    ));
    let stream_resp = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-stream".to_string()))
    });
    assert_eq!(stream_resp["ok"], Value::Bool(true));
    assert_eq!(stream_resp["result"]["subscribed"], Value::Bool(true));

    daemon.send(request(
        "req-submit",
        Some(&session_id),
        "submit_turn",
        json!({ "input": "tool:list_dir dir_path=." }),
    ));
    let submit_resp = daemon.recv_until(Duration::from_secs(20), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-submit".to_string()))
    });
    assert_eq!(submit_resp["ok"], Value::Bool(true));
    let turn_id = submit_resp["result"]["turn_id"]
        .as_str()
        .expect("turn_id missing")
        .to_string();

    let mut seen_turn_started = false;
    let mut seen_tool_started = false;
    let mut seen_tool_completed = false;
    let mut seen_turn_completed = false;

    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && !seen_turn_completed {
        let msg = daemon
            .rx
            .recv_timeout(Duration::from_millis(500))
            .expect("timed out waiting for event");
        if msg.get("kind") != Some(&Value::String("event".to_string())) {
            continue;
        }
        if msg.get("turn_id") != Some(&Value::String(turn_id.clone())) {
            continue;
        }
        let event_type = msg
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match event_type {
            "turn_started" => seen_turn_started = true,
            "tool_call_started" => seen_tool_started = true,
            "tool_call_completed" => seen_tool_completed = true,
            "turn_completed" => seen_turn_completed = true,
            _ => {}
        }
    }

    assert!(seen_turn_started, "missing turn_started event");
    assert!(seen_tool_started, "missing tool_call_started event");
    assert!(seen_tool_completed, "missing tool_call_completed event");
    assert!(seen_turn_completed, "missing turn_completed event");

    daemon.send(request(
        "req-shutdown",
        Some(&session_id),
        "shutdown_session",
        json!({}),
    ));
    let shutdown_resp = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-shutdown".to_string()))
    });
    assert_eq!(shutdown_resp["ok"], Value::Bool(true));
    assert_eq!(shutdown_resp["result"]["closed"], Value::Bool(true));
}

#[test]
fn m2_invalid_request_returns_error() {
    let mut daemon = DaemonHarness::start();

    daemon.send(json!({
        "kind": "request",
        "request_id": "req-invalid",
        "method": "start_session",
        "params": {}
    }));

    let resp = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-invalid".to_string()))
    });

    assert_eq!(resp["ok"], Value::Bool(false));
    assert_eq!(
        resp["error"]["code"],
        Value::String("INVALID_REQUEST".to_string())
    );
}

#[test]
fn m3_stream_events_arrive_before_long_turn_completes() {
    let mut daemon = DaemonHarness::start();

    daemon.send(request("req-start-3", None, "start_session", json!({})));
    let start_resp = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-start-3".to_string()))
    });
    let session_id = start_resp["result"]["session_id"]
        .as_str()
        .expect("session_id missing")
        .to_string();

    daemon.send(request(
        "req-stream-3",
        Some(&session_id),
        "stream_events",
        json!({}),
    ));
    let stream_resp = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-stream-3".to_string()))
    });
    assert_eq!(stream_resp["ok"], Value::Bool(true));

    let submit_started_at = Instant::now();
    daemon.send(request(
        "req-submit-3",
        Some(&session_id),
        "submit_turn",
        json!({ "input": "tool:shell cmd='sleep 2'" }),
    ));
    let submit_resp = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("response".to_string()))
            && msg.get("request_id") == Some(&Value::String("req-submit-3".to_string()))
    });
    assert_eq!(submit_resp["ok"], Value::Bool(true));
    let turn_id = submit_resp["result"]["turn_id"]
        .as_str()
        .expect("turn_id missing")
        .to_string();

    let early_event_deadline = Instant::now() + Duration::from_millis(900);
    let mut saw_tool_started_early = false;
    while Instant::now() < early_event_deadline {
        let Ok(msg) = daemon.rx.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        if msg.get("kind") != Some(&Value::String("event".to_string())) {
            continue;
        }
        if msg.get("turn_id") != Some(&Value::String(turn_id.clone())) {
            continue;
        }
        if msg.get("event_type") == Some(&Value::String("tool_call_started".to_string())) {
            saw_tool_started_early = true;
            break;
        }
    }
    assert!(
        saw_tool_started_early,
        "expected tool_call_started before long-running turn completed"
    );

    let done_msg = daemon.recv_until(Duration::from_secs(8), |msg| {
        msg.get("kind") == Some(&Value::String("event".to_string()))
            && msg.get("turn_id") == Some(&Value::String(turn_id.clone()))
            && msg.get("event_type") == Some(&Value::String("turn_completed".to_string()))
    });
    assert_eq!(
        done_msg.get("event_type"),
        Some(&Value::String("turn_completed".to_string()))
    );
    assert!(
        submit_started_at.elapsed() >= Duration::from_millis(1500),
        "long-running turn finished unexpectedly fast"
    );
}
