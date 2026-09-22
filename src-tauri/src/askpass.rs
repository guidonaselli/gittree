use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AskpassPromptType {
    HostKey,
    Passphrase,
    Username,
    TwoFactor,
    Password,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskpassPromptPayload {
    pub id: String,
    pub prompt: String,
    pub prompt_type: AskpassPromptType,
}

pub struct AskpassServer {
    pub socket_path: PathBuf,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Option<String>>>>>,
}

impl AskpassServer {
    pub fn dummy() -> Arc<Self> {
        Arc::new(Self {
            socket_path: PathBuf::new(),
            pending: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn start(app_handle: AppHandle) -> std::io::Result<Arc<Self>> {
        let pid = std::process::id();
        let rand_id: u32 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let socket_path = std::env::temp_dir().join(format!("gittree-askpass-{pid}-{rand_id}.sock"));

        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        let std_listener = std::os::unix::net::UnixListener::bind(&socket_path)?;
        std_listener.set_nonblocking(true)?;

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let counter = Arc::new(AtomicU64::new(1));

        let server = Arc::new(Self {
            socket_path: socket_path.clone(),
            pending: pending.clone(),
        });

        let app_handle_clone = app_handle.clone();
        let pending_clone = pending.clone();
        let counter_clone = counter.clone();

        tauri::async_runtime::spawn(async move {
            let listener = match UnixListener::from_std(std_listener) {
                Ok(l) => l,
                Err(_) => return,
            };
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let app = app_handle_clone.clone();
                        let pending = pending_clone.clone();
                        let id = format!("askpass-{}", counter_clone.fetch_add(1, Ordering::SeqCst));

                        tokio::spawn(async move {
                            let (reader, mut writer) = stream.into_split();
                            let mut buf_reader = BufReader::new(reader);
                            let mut line = String::new();

                            if buf_reader.read_line(&mut line).await.is_err() {
                                return;
                            }

                            let prompt = line.trim().to_string();
                            let prompt_type = classify_prompt(&prompt);

                            let (tx, rx) = oneshot::channel();
                            {
                                let mut map = pending.lock().await;
                                map.insert(id.clone(), tx);
                            }

                            let payload = AskpassPromptPayload {
                                id: id.clone(),
                                prompt: prompt.clone(),
                                prompt_type,
                            };

                            let _ = app.emit("askpass:prompt", payload);

                            // Wait for user input or timeout (120s)
                            let result = tokio::time::timeout(Duration::from_secs(120), rx).await;

                            // Clean up pending
                            {
                                let mut map = pending.lock().await;
                                map.remove(&id);
                            }

                            match result {
                                Ok(Ok(Some(response))) => {
                                    let _ = writer.write_all(response.as_bytes()).await;
                                    let _ = writer.write_all(b"\n").await;
                                    let _ = writer.flush().await;
                                }
                                _ => {
                                    // User cancelled or timeout: close without writing so client exits with 1
                                    let _ = writer.shutdown().await;
                                }
                            }
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(server)
    }

    pub fn askpass_env(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();
        if !self.socket_path.as_os_str().is_empty() {
            if let Ok(exe) = std::env::current_exe() {
                let exe_str = exe.to_string_lossy().to_string();
                env.push(("GIT_ASKPASS".to_string(), exe_str.clone()));
                env.push(("SSH_ASKPASS".to_string(), exe_str));
                env.push(("SSH_ASKPASS_REQUIRE".to_string(), "force".to_string()));
                env.push((
                    "GITTREE_ASKPASS_SOCKET".to_string(),
                    self.socket_path.to_string_lossy().to_string(),
                ));
            }
        }
        env
    }

    pub async fn submit_response(&self, id: &str, response: String) -> bool {
        let mut map = self.pending.lock().await;
        if let Some(tx) = map.remove(id) {
            tx.send(Some(response)).is_ok()
        } else {
            false
        }
    }

    pub async fn cancel_response(&self, id: &str) -> bool {
        let mut map = self.pending.lock().await;
        if let Some(tx) = map.remove(id) {
            tx.send(None).is_ok()
        } else {
            false
        }
    }
}

impl Drop for AskpassServer {
    fn drop(&mut self) {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

fn classify_prompt(prompt: &str) -> AskpassPromptType {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("authenticity of host")
        || lower.contains("fingerprint")
        || lower.contains("continue connecting (yes/no")
    {
        AskpassPromptType::HostKey
    } else if lower.contains("passphrase") {
        AskpassPromptType::Passphrase
    } else if lower.contains("username") {
        AskpassPromptType::Username
    } else if lower.contains("2fa")
        || lower.contains("one-time")
        || lower.contains("two-factor")
        || lower.contains("verification code")
        || lower.contains("token")
    {
        AskpassPromptType::TwoFactor
    } else {
        AskpassPromptType::Password
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_prompts() {
        assert_eq!(
            classify_prompt("The authenticity of host 'github.com' can't be established. Are you sure you want to continue connecting (yes/no)?"),
            AskpassPromptType::HostKey
        );
        assert_eq!(
            classify_prompt("Enter passphrase for key '/home/user/.ssh/id_ed25519':"),
            AskpassPromptType::Passphrase
        );
        assert_eq!(
            classify_prompt("Username for 'https://github.com':"),
            AskpassPromptType::Username
        );
        assert_eq!(
            classify_prompt("Enter your 2FA verification code:"),
            AskpassPromptType::TwoFactor
        );
        assert_eq!(
            classify_prompt("Password for 'https://alice@github.com':"),
            AskpassPromptType::Password
        );
    }
}
