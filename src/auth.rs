use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

use greetd_ipc::codec::SyncCodec;
use greetd_ipc::{AuthMessageType, ErrorType, Request, Response};

use crate::i18n::STRINGS;
use crate::system::sessions::SessionKind;

#[derive(Debug, Clone)]
pub struct AuthParams {
    pub username: String,
    pub password: String,
    pub session_cmd: Vec<String>,
    pub session_kind: SessionKind,
    pub extra_env: HashMap<String, String>,
    pub mock: bool,
}

pub fn authenticate_and_start(params: &AuthParams) -> Result<(), String> {
    if params.password.is_empty() {
        return Err(STRINGS.err_empty_password.to_string());
    }
    if params.session_cmd.is_empty() {
        return Err(STRINGS.err_session_command.to_string());
    }

    if params.mock {
        return run_mock_auth(params);
    }

    let sock_path =
        std::env::var("GREETD_SOCK").map_err(|_| STRINGS.err_no_socket.to_string())?;
    if sock_path.trim().is_empty() {
        return Err(STRINGS.err_no_socket.to_string());
    }

    let mut stream = UnixStream::connect(&sock_path)
        .map_err(|e| format!("Failed to connect to greetd socket ({sock_path}): {e}"))?;

    if let Err(err) = perform_greetd_handshake(&mut stream, params) {
        // Reset greetd session state so a retry starts a clean PAM conversation.
        let _ = Request::CancelSession.write_to(&mut stream);
        return Err(err);
    }

    Ok(())
}

fn perform_greetd_handshake(stream: &mut UnixStream, params: &AuthParams) -> Result<(), String> {
    Request::CreateSession {
        username: params.username.clone(),
    }
    .write_to(stream)
    .map_err(|e| format!("Failed to write CreateSession: {e}"))?;

    let mut password_sent = false;

    loop {
        let resp = Response::read_from(stream)
            .map_err(|e| format!("Failed to read response from greetd: {e}"))?;

        match resp {
            Response::AuthMessage {
                auth_message_type,
                auth_message,
            } => match auth_message_type {
                AuthMessageType::Secret => {
                    if password_sent {
                        return Err(format_pam_error(&auth_message));
                    }
                    password_sent = true;
                    Request::PostAuthMessageResponse {
                        response: Some(params.password.clone()),
                    }
                    .write_to(stream)
                    .map_err(|e| format!("Failed to send password to greetd: {e}"))?;
                }
                AuthMessageType::Visible => {
                    Request::PostAuthMessageResponse {
                        response: Some(params.username.clone()),
                    }
                    .write_to(stream)
                    .map_err(|e| format!("Failed to reply to visible prompt: {e}"))?;
                }
                AuthMessageType::Info => {
                    Request::PostAuthMessageResponse { response: None }
                        .write_to(stream)
                        .map_err(|e| format!("Failed to acknowledge PAM info: {e}"))?;
                }
                AuthMessageType::Error => {
                    return Err(format_pam_error(&auth_message));
                }
            },
            Response::Success => break,
            Response::Error {
                error_type,
                description,
            } => {
                return Err(format_greetd_error(error_type, &description));
            }
        }
    }

    let env = build_session_env(params.session_kind, &params.extra_env);
    Request::StartSession {
        cmd: params.session_cmd.clone(),
        env,
    }
    .write_to(stream)
    .map_err(|e| format!("Failed to send StartSession: {e}"))?;

    match Response::read_from(stream)
        .map_err(|e| format!("Failed to confirm StartSession: {e}"))?
    {
        Response::Success => Ok(()),
        Response::Error {
            error_type,
            description,
        } => Err(format_greetd_error(error_type, &description)),
        Response::AuthMessage { auth_message, .. } => Err(format!(
            "Unexpected authentication prompt while starting session: {auth_message}"
        )),
    }
}

fn build_session_env(
    kind: SessionKind,
    extra_env: &HashMap<String, String>,
) -> Vec<String> {
    let mut env = Vec::new();
    match kind {
        SessionKind::Wayland => {
            env.push("XDG_SESSION_TYPE=wayland".to_string());
        }
        SessionKind::X11 => {
            env.push("XDG_SESSION_TYPE=x11".to_string());
        }
    }
    for (k, v) in extra_env {
        env.push(format!("{k}={v}"));
    }
    env
}

fn format_greetd_error(error_type: ErrorType, description: &str) -> String {
    match error_type {
        ErrorType::AuthError => format_pam_error(description),
        ErrorType::Error => {
            if description.trim().is_empty() {
                "greetd returned an internal session error.".to_string()
            } else {
                format!("Session error: {}", description.trim())
            }
        }
    }
}

pub fn format_pam_error(raw: &str) -> String {
    let lower = raw.to_lowercase();

    if lower.contains("maxtries") || lower.contains("too many") || lower.contains("faillock") {
        return STRINGS.err_max_tries.to_string();
    }
    if lower.contains("acct_expired")
        || lower.contains("account locked")
        || lower.contains("account has expired")
        || lower.contains("nologin")
    {
        return STRINGS.err_account_locked.to_string();
    }
    if lower.contains("new_authtok_reqd") || lower.contains("password expired") {
        return STRINGS.err_password_expired.to_string();
    }
    if lower.is_empty()
        || lower.contains("authentication failure")
        || lower.contains("auth_err")
        || lower.contains("incorrect password")
        || lower.contains("wrong password")
    {
        return STRINGS.err_wrong_password.to_string();
    }

    format!("{} ({})", STRINGS.err_wrong_password, raw.trim())
}

fn run_mock_auth(params: &AuthParams) -> Result<(), String> {
    thread::sleep(Duration::from_millis(220));
    match params.password.as_str() {
        "wrong" | "fail" => Err(format_pam_error("PAM authentication failure")),
        "locked" => Err(format_pam_error("account locked via pam_faillock")),
        "expired" => Err(format_pam_error("PAM_NEW_AUTHTOK_REQD")),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_pam_errors_to_readable_messages() {
        let auth_err = format_pam_error("PAM authentication failure");
        assert!(auth_err.contains("Incorrect password"));

        let locked = format_pam_error("account locked via pam_faillock");
        assert!(locked.contains("Too many failed attempts") || locked.contains("locked"));
    }

    #[test]
    fn rejects_empty_password_before_connecting() {
        let params = AuthParams {
            username: "koresent".to_string(),
            password: String::new(),
            session_cmd: vec!["umbriel".to_string()],
            session_kind: SessionKind::Wayland,
            extra_env: HashMap::new(),
            mock: true,
        };
        let err = authenticate_and_start(&params).unwrap_err();
        assert!(err.contains("Enter your password"));
    }
}
