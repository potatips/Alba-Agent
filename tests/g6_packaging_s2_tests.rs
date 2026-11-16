use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[tokio::test]
async fn test_tauri_shell_scripted_launch_and_process_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web/out");

    let bin_path = if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(target_dir).join("debug/alba-agent")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/alba-agent")
    };

    if !bin_path.exists() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--package")
            .arg("alba-agent-desktop")
            .arg("--bin")
            .arg("alba-agent")
            .current_dir(&manifest_dir);
        if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
            cmd.env("CARGO_TARGET_DIR", target_dir);
        }
        let status = cmd.status().expect("failed to build alba-agent binary");
        assert!(status.success(), "cargo build alba-agent failed");
    }

    assert!(
        bin_path.exists(),
        "alba-agent binary must exist at {:?}",
        bin_path
    );

    let mut child = Command::new(&bin_path)
        .env("ALBA_DATA_DIR", &data_dir)
        .env("ALBA_STATIC_DIR", &static_dir)
        .spawn()
        .expect("failed to spawn alba-agent");

    let pid = child.id();
    let port_file = data_dir.join("backend_port");

    let mut port: Option<u16> = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if port_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&port_file) {
                let trimmed = content.trim();
                if let Ok(p) = trimmed.parse::<u16>() {
                    port = Some(p);
                    break;
                }
            }
        }
    }

    assert!(port.is_some(), "timed out waiting for backend_port file");
    let port_val = port.unwrap();

    let client = reqwest::Client::new();
    let health_res = client
        .get(format!("http://127.0.0.1:{}/health", port_val))
        .send()
        .await
        .expect("failed to call health endpoint");
    assert_eq!(health_res.status(), reqwest::StatusCode::OK);
    let health_body: serde_json::Value = health_res.json().await.unwrap();
    assert_eq!(health_body["status"], "ok");

    let root_res = client
        .get(format!("http://127.0.0.1:{}/", port_val))
        .send()
        .await
        .expect("failed to call root UI endpoint");
    assert_eq!(root_res.status(), reqwest::StatusCode::OK);
    let root_body = root_res.text().await.unwrap();
    assert!(root_body.contains("<!DOCTYPE html>"));

    let ps_output = Command::new("ps")
        .args(["-ef"])
        .output()
        .expect("failed to run ps");
    let ps_str = String::from_utf8_lossy(&ps_output.stdout);

    for line in ps_str.lines() {
        if line.contains(&pid.to_string()) && !line.contains("ps -ef") {
            let lower = line.to_lowercase();
            assert!(
                !lower.contains("python"),
                "found python process in process tree: {}",
                line
            );
            assert!(
                !lower.contains("node"),
                "found node process in process tree: {}",
                line
            );
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}
