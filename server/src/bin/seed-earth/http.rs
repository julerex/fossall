use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context};
use serde_json::Value;

pub const SEED_USER_AGENT: &str =
    "fossall-earth-seed/0.1 (+https://fossall.com; https://github.com/julerex/fossall)";

pub async fn get_json(url: &str) -> anyhow::Result<Value> {
    let mut last_err = None;
    for attempt in 0..4 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 * (1 << attempt))).await;
        }
        match fetch_once(url).await {
            Ok(v) => return Ok(v),
            Err(err) => last_err = Some(err.to_string()),
        }
    }
    bail!("GET failed after retries: {}", last_err.unwrap_or_default())
}

async fn fetch_once(url: &str) -> anyhow::Result<Value> {
    let url = url.to_string();
    let url_for_err = url.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        let output = Command::new("curl")
            .args([
                "-fsSL",
                "-A",
                SEED_USER_AGENT,
                "--max-time",
                "120",
                "--retry",
                "1",
                &url,
            ])
            .output()
            .context("run curl")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("curl {} failed: {}", url, truncate(&stderr, 200));
        }
        Ok(output.stdout)
    })
    .await
    .context("join curl")??;
    serde_json::from_slice(&bytes).with_context(|| format!("parse JSON from {url_for_err}"))
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

pub fn env_u32(name: &str) -> Option<u32> {
    std::env::var(name).ok().and_then(|s| s.parse().ok())
}

pub fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
