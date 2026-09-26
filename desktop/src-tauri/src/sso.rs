//! Loopback listener for the single sign-on redirect.
//!
//! A desktop app cannot receive an HTTPS redirect, so the OIDC flow uses the
//! standard native pattern: the provider redirects to `http://127.0.0.1:<port>`,
//! which this module answers exactly once before shutting down.
//!
//! The authorization code never passes through the clipboard or a pasted URL,
//! and the listener binds only to loopback so nothing off-machine can reach it.

use std::time::Duration;

use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Port the provider must be configured to redirect to.
///
/// Fixed rather than ephemeral because an identity provider requires redirect
/// URIs to be registered ahead of time.
pub const SSO_REDIRECT_PORT: u16 = 18081;

/// Abandoned if the artist never finishes signing in, so no listener leaks.
const REDIRECT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Cap on the request line we will read, which is all we need.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

#[derive(Debug, Serialize)]
pub struct SsoRedirect {
    pub code: String,
    pub state: String,
}

/// Waits for the provider's redirect and returns the authorization code.
#[tauri::command]
pub async fn await_sso_redirect() -> Result<SsoRedirect, String> {
    let listener = TcpListener::bind(("127.0.0.1", SSO_REDIRECT_PORT))
        .await
        .map_err(|error| {
            format!(
                "Could not listen for the sign-in redirect on port {SSO_REDIRECT_PORT}: {error}"
            )
        })?;

    let accepted = tokio::time::timeout(REDIRECT_TIMEOUT, accept_redirect(&listener)).await;
    match accepted {
        Ok(result) => result,
        Err(_) => Err("Sign-in timed out before the browser returned.".to_string()),
    }
}

async fn accept_redirect(listener: &TcpListener) -> Result<SsoRedirect, String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|error| format!("Sign-in redirect failed: {error}"))?;

        let mut buffer = vec![0u8; MAX_REQUEST_BYTES];
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|error| format!("Could not read the sign-in redirect: {error}"))?;
        let request = String::from_utf8_lossy(&buffer[..read]);
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or_default()
            .to_string();

        // Browsers request /favicon.ico alongside the redirect; answering and
        // continuing avoids mistaking it for a failed sign-in.
        if target.starts_with("/favicon") {
            let _ = respond(&mut stream, "404 Not Found", "Not found").await;
            continue;
        }

        let params = query_params(&target);
        if let Some(error) = params.iter().find(|(key, _)| key == "error") {
            let detail = params
                .iter()
                .find(|(key, _)| key == "error_description")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| error.1.clone());
            let _ = respond(&mut stream, "200 OK", &page("Sign-in was declined.")).await;
            return Err(format!(
                "Your identity provider declined the sign-in: {detail}"
            ));
        }

        let code = params
            .iter()
            .find(|(key, _)| key == "code")
            .map(|(_, value)| value.clone());
        let state = params
            .iter()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.clone());

        match (code, state) {
            (Some(code), Some(state)) => {
                let _ = respond(
                    &mut stream,
                    "200 OK",
                    &page("Signed in. You can close this tab and return to OpenAsset Depot."),
                )
                .await;
                return Ok(SsoRedirect { code, state });
            }
            _ => {
                let _ = respond(
                    &mut stream,
                    "400 Bad Request",
                    &page("Missing sign-in details."),
                )
                .await;
                return Err("The sign-in redirect was missing its authorization code.".to_string());
            }
        }
    }
}

async fn respond(
    stream: &mut tokio::net::TcpStream,
    status: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

fn page(message: &str) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>OpenAsset Depot</title>\
         <body style=\"font-family:-apple-system,Segoe UI,sans-serif;display:grid;place-items:center;height:100vh;margin:0;color:#1d1e20\">\
         <p>{message}</p></body>"
    )
}

/// Parses `key=value` pairs from a request target, percent-decoding values.
fn query_params(target: &str) -> Vec<(String, String)> {
    let Some((_, query)) = target.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_string(), percent_decode(value)))
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                match u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    Ok(decoded) => {
                        out.push(decoded);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{percent_decode, query_params};

    #[test]
    fn authorization_parameters_are_decoded() {
        let params = query_params("/callback?code=abc%2F123&state=x+y");
        assert_eq!(params[0], ("code".to_string(), "abc/123".to_string()));
        assert_eq!(params[1], ("state".to_string(), "x y".to_string()));
    }

    #[test]
    fn a_target_without_a_query_yields_nothing() {
        assert!(query_params("/callback").is_empty());
    }

    #[test]
    fn malformed_escapes_are_preserved_rather_than_dropped() {
        assert_eq!(percent_decode("100%"), "100%");
    }
}
