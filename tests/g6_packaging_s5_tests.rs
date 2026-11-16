use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[tokio::test]
async fn test_release_bundle_execution_and_process_tree() {
    let bundle_path = if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(target_dir).join("release/bundle/macos/Alba Agent.app")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/bundle/macos/Alba Agent.app")
    };

    if !bundle_path.exists() {
        eprintln!(
            "bundle not found at {:?}, skipping release bundle test",
            bundle_path
        );
        return;
    }

    let bin_path = bundle_path.join("Contents/MacOS/alba-agent");
    assert!(
        bin_path.exists(),
        "bundle executable must exist at {:?}",
        bin_path
    );

    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();

    let mut child = Command::new(&bin_path)
        .arg("-ApplePersistenceIgnoreState")
        .arg("YES")
        .env("ALBA_DATA_DIR", &data_dir)
        .spawn()
        .expect("failed to spawn bundle executable");

    let pid = child.id();
    let port_file = data_dir.join("backend_port");

    let mut port: Option<u16> = None;
    for _ in 0..120 {
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

    assert!(
        port.is_some(),
        "timed out waiting for backend_port file from bundle"
    );
    let port_val = port.unwrap();

    let client = reqwest::Client::new();
    let health_res = client
        .get(format!("http://127.0.0.1:{}/health", port_val))
        .send()
        .await
        .expect("failed to call health endpoint on bundle");
    assert_eq!(health_res.status(), reqwest::StatusCode::OK);
    let health_body: serde_json::Value = health_res.json().await.unwrap();
    assert_eq!(health_body["status"], "ok");

    let root_res = client
        .get(format!("http://127.0.0.1:{}/", port_val))
        .send()
        .await
        .expect("failed to call root UI endpoint on bundle");
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
                "found python process in bundle process tree: {}",
                line
            );
            assert!(
                !lower.contains("node"),
                "found node process in bundle process tree: {}",
                line
            );
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}
