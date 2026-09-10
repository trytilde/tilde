//! Exercise the real environment/file chain in subprocesses, without changing global test env.
use std::{
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn credential_chain() {
    if let Ok(mode) = std::env::var("TILDE_CREDENTIAL_FIXTURE") {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(client_aws_config::load("us-east-1"));
        if mode == "missing" || mode == "empty" {
            assert!(matches!(
                result,
                Err(client_aws_config::Error::CredentialsUnavailable)
            ));
        } else {
            let config = result.unwrap();
            let prefix = if mode == "env" {
                "env"
            } else if mode == "ecs" {
                "ecs"
            } else {
                "file"
            };
            assert_eq!(config.credentials.access_key_id, format!("{prefix}-access"));
            assert_eq!(
                config.credentials.secret_access_key,
                format!("{prefix}-secret")
            );
            assert_eq!(
                config.credentials.session_token.as_deref(),
                Some(format!("{prefix}-token").as_str())
            );
            assert!(!format!("{config:?}").contains("secret"));
            assert!(!format!("{config:?}").contains("token"));
        }
        return;
    }
    let temp = std::env::temp_dir().join(format!(
        "tilde-credentials-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&temp).unwrap();
    let file = temp.join("credentials");
    std::fs::write(&file, "[default]\naws_access_key_id=unused\naws_secret_access_key=unused\n[fixture]\naws_access_key_id=file-access\naws_secret_access_key=file-secret\naws_session_token=file-token\n").unwrap();
    for mode in ["env", "ecs", "file", "missing", "empty"] {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", "credential_chain", "--nocapture"])
            .env_clear()
            .env("TILDE_CREDENTIAL_FIXTURE", mode)
            .env("HOME", &temp)
            .env("AWS_SHARED_CREDENTIALS_FILE", &file)
            .env("AWS_PROFILE", "fixture");
        if mode == "env" || mode == "empty" {
            child
                .env("AWS_ACCESS_KEY_ID", "env-access")
                .env(
                    "AWS_SECRET_ACCESS_KEY",
                    if mode == "empty" { "" } else { "env-secret" },
                )
                .env("AWS_SESSION_TOKEN", "env-token");
        }
        if mode == "missing" {
            child.env("AWS_SHARED_CREDENTIALS_FILE", temp.join("absent"));
        }
        let server = if mode == "ecs" {
            use std::io::{Read, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            child
                .env(
                    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
                    format!("http://{}/credentials", listener.local_addr().unwrap()),
                )
                .env("AWS_CONTAINER_AUTHORIZATION_TOKEN", "fixture-authorization");
            Some(std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let mut bytes = [0; 1024];
                    let size = stream.read(&mut bytes).unwrap();
                    assert!(size > 0);
                    request.extend_from_slice(&bytes[..size]);
                }
                assert!(
                    String::from_utf8(request)
                        .unwrap()
                        .contains("authorization: fixture-authorization")
                );
                let body = r#"{"AccessKeyId":"ecs-access","SecretAccessKey":"ecs-secret","Token":"ecs-token"}"#;
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            }))
        } else {
            None
        };
        let result = child.output().unwrap();
        assert!(
            result.status.success(),
            "credential source {mode} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        if let Some(server) = server {
            server.join().unwrap();
        }
    }
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(temp).unwrap();
}
