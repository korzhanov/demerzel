//! Google Calendar OAuth (desktop loopback + PKCE) and event synchronization.

use super::calendar::ActivityRecord;
use keyring::Entry;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use url::Url;

const SERVICE: &str = "app.demerzel.orchestrator.google-calendar";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/calendar.calendarlist.readonly";

fn form_request(
    client: &Client,
    endpoint: &str,
    fields: &[(&str, &str)],
) -> reqwest::blocking::RequestBuilder {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish();
    client
        .post(endpoint)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CalendarItem {
    pub id: String,
    pub summary: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarListResponse {
    items: Vec<CalendarItem>,
    #[serde(default)]
    next_page_token: Option<String>,
}

fn credential(client_id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, client_id).map_err(|e| e.to_string())
}

pub fn connected(client_id: &str) -> bool {
    credential(client_id)
        .and_then(|e| e.get_password().map_err(|e| e.to_string()))
        .is_ok()
}

pub fn start_auth(
    client_id: String,
    verifier: String,
    challenge: String,
    state: String,
) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Не удалось открыть локальный callback OAuth: {e}"))?;
    let redirect = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().map_err(|e| e.to_string())?.port()
    );
    let mut url =
        Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state);
    thread::spawn(move || {
        if let Err(error) = receive_auth(listener, &client_id, &redirect, &verifier, &state) {
            eprintln!("[calendar/google] OAuth callback failed: {error}");
        }
    });
    Ok(url.to_string())
}

fn receive_auth(
    listener: TcpListener,
    client_id: &str,
    redirect: &str,
    verifier: &str,
    expected_state: &str,
) -> Result<(), String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(error) => {
                return Err(if error.kind() == std::io::ErrorKind::WouldBlock {
                    "Google OAuth callback timed out".into()
                } else {
                    error.to_string()
                });
            }
        }
    };
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut request = [0u8; 8192];
    let count = stream.read(&mut request).map_err(|e| e.to_string())?;
    let first = String::from_utf8_lossy(&request[..count])
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let path = first
        .split_whitespace()
        .nth(1)
        .ok_or("Invalid OAuth callback")?;
    let callback = Url::parse(&format!("{redirect}{}", path.trim_start_matches('/')))
        .map_err(|e| e.to_string())?;
    let params: std::collections::HashMap<_, _> = callback.query_pairs().into_owned().collect();
    let body = if params.get("state").map(String::as_str) != Some(expected_state) {
        "Вход не подтверждён. Закройте страницу и повторите подключение в Demerzel."
    } else if params.contains_key("error") {
        return respond(&mut stream, "Google не завершил вход. Вернитесь в Demerzel и повторите подключение.");
    } else {
        "Google Calendar подключён. Можно вернуться в Demerzel."
    };
    if params.get("state").map(String::as_str) != Some(expected_state) {
        respond(&mut stream, body)?;
        return Err("OAuth state mismatch".into());
    }
    let code = params
        .get("code")
        .ok_or("Google did not return an authorization code")?;
    respond(&mut stream, body)?;
    let client = Client::new();
    let token: TokenResponse = form_request(
        &client,
        "https://oauth2.googleapis.com/token",
        &[
            ("client_id", client_id),
            ("code", code),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect),
        ],
    )
    .send()
    .map_err(|e| e.to_string())?
    .error_for_status()
    .map_err(|e| e.to_string())?
    .json()
    .map_err(|e| e.to_string())?;
    if let Some(refresh) = token.refresh_token {
        credential(client_id)?
            .set_password(&refresh)
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Google не вернул refresh token. Отключите доступ Demerzel в настройках аккаунта Google и повторите вход.".into())
    }
}

fn respond(stream: &mut std::net::TcpStream, message: &str) -> Result<(), String> {
    let body =
        format!("<!doctype html><meta charset=utf-8><title>Demerzel</title><p>{message}</p>");
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).map_err(|e| e.to_string())
}

fn access_token(client_id: &str) -> Result<String, String> {
    let refresh = credential(client_id)?
        .get_password()
        .map_err(|_| "Google Calendar не подключён".to_string())?;
    let client = Client::new();
    let token: TokenResponse = form_request(
        &client,
        "https://oauth2.googleapis.com/token",
        &[
            ("client_id", client_id),
            ("refresh_token", &refresh),
            ("grant_type", "refresh_token"),
        ],
    )
    .send()
    .map_err(|e| e.to_string())?
    .error_for_status()
    .map_err(|e| e.to_string())?
    .json()
    .map_err(|e| e.to_string())?;
    let _ = token.expires_in;
    Ok(token.access_token)
}

pub fn list_calendars(client_id: &str) -> Result<Vec<CalendarItem>, String> {
    let token = access_token(client_id)?;
    let client = Client::new();
    let mut page: Option<String> = None;
    let mut result = Vec::new();
    loop {
        let mut url = Url::parse("https://www.googleapis.com/calendar/v3/users/me/calendarList")
            .map_err(|e| e.to_string())?;
        url.query_pairs_mut()
            .append_pair("minAccessRole", "writer")
            .append_pair("maxResults", "250");
        if let Some(ref value) = page {
            url.query_pairs_mut().append_pair("pageToken", value);
        }
        let response: CalendarListResponse = client
            .get(url)
            .bearer_auth(&token)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
        result.extend(response.items);
        page = response.next_page_token;
        if page.is_none() {
            break;
        }
    }
    Ok(result)
}

pub fn disconnect(client_id: &str) -> Result<(), String> {
    if let Ok(entry) = credential(client_id) {
        if let Ok(refresh) = entry.get_password() {
            let _ = form_request(
                &Client::new(),
                "https://oauth2.googleapis.com/revoke",
                &[("token", &refresh)],
            )
            .send();
        }
        let _ = entry.delete_credential();
    }
    Ok(())
}

pub fn upsert(client_id: &str, calendar_id: &str, record: &ActivityRecord) -> Result<(), String> {
    let token = access_token(client_id)?;
    let digest = Sha256::digest(format!("{}@demerzel", record.uid).as_bytes());
    const BASE32HEX: &[u8; 32] = b"0123456789abcdefghijklmnopqrstuv";
    let mut event_id = String::with_capacity(52);
    let (mut buffer, mut bits) = (0u32, 0u8);
    for byte in digest {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            event_id.push(BASE32HEX[((buffer >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        event_id.push(BASE32HEX[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }
    let start =
        chrono::DateTime::parse_from_rfc3339(&record.started_at).map_err(|e| e.to_string())?;
    let mut end =
        chrono::DateTime::parse_from_rfc3339(&record.finished_at).map_err(|e| e.to_string())?;
    if end <= start {
        end = start + chrono::Duration::seconds(1);
    }
    let event = serde_json::json!({
        "id": event_id,
        "summary": record.summary,
        "description": record.description,
        "start": {"dateTime": start.to_rfc3339()},
        "end": {"dateTime": end.to_rfc3339()},
        "extendedProperties": {"private": {"demerzelUid": record.uid}}
    });
    let client = Client::new();
    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/{}/events/{}",
        urlencoding::encode(calendar_id),
        event_id
    );
    let existing = client
        .get(&url)
        .bearer_auth(&token)
        .send()
        .map_err(|e| e.to_string())?;
    let response = if existing.status().as_u16() == 404 {
        client
            .post(format!(
                "https://www.googleapis.com/calendar/v3/calendars/{}/events",
                urlencoding::encode(calendar_id)
            ))
            .bearer_auth(&token)
            .json(&event)
            .send()
    } else if existing.status().is_success() {
        client.patch(&url).bearer_auth(&token).json(&event).send()
    } else {
        return Err(existing
            .error_for_status()
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Google Calendar request failed".into()));
    };
    response
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
