// ERSM Remoto support mode (replica/architecture.md, «El broker de códigos»; contract in replica/api-m1.md).
// The portable client served by the portal is named ERSM-Remoto-Soporte_<ticket>.exe. In that mode it
// redeems the ticket once, publishes its ID and its one-time password sealed for the technician's key
// (re-publishing when the one-time password rotates), and every incoming login needs the user's click.
use base::config::keys;
use hbb_common::{
    bail,
    config::{self, Config},
    log, password_security as password,
    sodiumoxide::{
        crypto::{box_, sealedbox},
        randombytes,
    },
    tls::TlsType,
    tokio::{
        self,
        time::{sleep, Duration, Instant},
    },
    ResultType,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

const EXE_PREFIX: &str = "ersm-remoto-soporte";
const TICKET_PREFIX: &str = "ersm-remoto-soporte_";
const DEFAULT_API_URL: &str = "https://remoto.ersmgrupo.org";
const REGISTER_TIMEOUT: Duration = Duration::from_secs(180);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub fn api_url() -> &'static str {
    option_env!("ERSM_API_URL")
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_API_URL)
        .trim_end_matches('/')
}

// The portable packer extracts the real exe elsewhere and passes the downloaded file name in this variable.
fn exe_file_name() -> String {
    if let Ok(name) = std::env::var(crate::common::PORTABLE_APPNAME_RUNTIME_ENV_KEY) {
        return name;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

pub fn is_support_mode() -> bool {
    static SUPPORT: OnceLock<bool> = OnceLock::new();
    *SUPPORT.get_or_init(|| exe_file_name().to_ascii_lowercase().starts_with(EXE_PREFIX))
}

// Tolerates what browsers append to repeated downloads, e.g. "ERSM-Remoto-Soporte_<ticket> (1).exe".
pub fn ticket_from_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let start = lower.find(TICKET_PREFIX)? + TICKET_PREFIX.len();
    let ticket: String = lower[start..].chars().take(32).collect();
    if ticket.len() == 32 && ticket.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(ticket)
    } else {
        None
    }
}

// A correct password must not open the session by itself: the user always clicks «Permitir».
pub fn requires_click() -> bool {
    is_support_mode()
}

pub fn apply_support_settings() {
    if !is_support_mode() {
        return;
    }
    let mut settings = config::OVERWRITE_SETTINGS.write().unwrap();
    for (k, v) in [
        (keys::OPTION_VERIFICATION_METHOD, "use-temporary-password"),
        (keys::OPTION_TEMPORARY_PASSWORD_LENGTH, "10"),
    ] {
        settings.insert(k.to_owned(), v.to_owned());
    }
}

pub fn spawn_join() {
    if !is_support_mode() {
        return;
    }
    tokio::spawn(async {
        if let Err(e) = run_join().await {
            log::error!("ERSM support: {e}");
        }
    });
}

async fn run_join() -> ResultType<()> {
    let name = exe_file_name();
    let Some(ticket) = ticket_from_name(&name) else {
        bail!("no ticket in the executable name {name:?}");
    };
    wait_registered().await?;

    let nonce = crate::common::encode64(randombytes::randombytes(32))
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_owned();
    let redeem = json!({
        "ticket": ticket,
        "client_nonce": nonce,
        "client_info": client_info(),
    });
    let joined = post_with_retry("/api/v1/join/redeem", &redeem).await?;
    let session_id = field(&joined, "session_id")?;
    let join_secret = field(&joined, "join_secret")?;
    let tech_pubkey = crate::common::decode64(field(&joined, "tech_pubkey")?)?;
    let Some(tech_pubkey) = box_::PublicKey::from_slice(&tech_pubkey) else {
        bail!("invalid technician public key");
    };
    log::info!(
        "ERSM support: joined session {session_id} with {}",
        joined["tech_name"].as_str().unwrap_or("?")
    );

    let mut published = (String::new(), String::new());
    loop {
        let current = (Config::get_id(), password::temporary_password());
        if current != published && !current.0.is_empty() && !current.1.is_empty() {
            let sealed = sealedbox::seal(current.1.as_bytes(), &tech_pubkey);
            let attach = json!({
                "session_id": &session_id,
                "join_secret": &join_secret,
                "rustdesk_id": &current.0,
                "sealed_otp": crate::common::encode64(sealed),
            });
            match post_json("/api/v1/join/attach", &attach).await {
                Ok((204, _)) => {
                    log::info!("ERSM support: published ID and sealed one-time password");
                    published = current;
                }
                Ok((403 | 409, _)) => {
                    log::info!("ERSM support: session closed on the server");
                    return Ok(());
                }
                Ok((status, body)) => log::warn!("ERSM support: attach answered {status}: {body}"),
                Err(e) => log::warn!("ERSM support: attach failed: {e}"),
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
}

async fn wait_registered() -> ResultType<()> {
    let deadline = Instant::now() + REGISTER_TIMEOUT;
    while Instant::now() < deadline {
        if !Config::get_id().is_empty()
            && Config::get_key_confirmed()
            && config::get_online_state() > 0
        {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    bail!(
        "not registered on {} after {REGISTER_TIMEOUT:?}",
        crate::ersm::RENDEZVOUS_SERVER
    )
}

fn client_info() -> Value {
    json!({
        "computer_name": crate::common::hostname(),
        "windows_user": crate::common::username(),
        "domain": std::env::var("USERDOMAIN").unwrap_or_default(),
        "os_version": hbb_common::whoami::distro(),
        "client_version": crate::VERSION,
    })
}

fn field(v: &Value, name: &str) -> ResultType<String> {
    match v[name].as_str() {
        Some(s) if !s.is_empty() => Ok(s.to_owned()),
        _ => bail!("missing {name} in the server answer"),
    }
}

// Redeem is idempotent for the same client_nonce, so network errors and 5xx are retried.
async fn post_with_retry(path: &str, body: &Value) -> ResultType<Value> {
    let mut wait = Duration::from_secs(2);
    for attempt in 1..=5 {
        match post_json(path, body).await {
            Ok((200, v)) => return Ok(v),
            Ok((status, v)) if status < 500 => {
                bail!(
                    "{path} answered {status}: {}",
                    v["message"].as_str().unwrap_or("?")
                )
            }
            Ok((status, _)) => {
                log::warn!("ERSM support: {path} answered {status} (attempt {attempt})")
            }
            Err(e) => log::warn!("ERSM support: {path} failed (attempt {attempt}): {e}"),
        }
        sleep(wait).await;
        wait *= 2;
    }
    bail!("{path} kept failing")
}

// HTTPS only with certificate checks; plain HTTP is accepted only when a test build points ERSM_API_URL at it.
async fn post_json(path: &str, body: &Value) -> ResultType<(u16, Value)> {
    let url = format!("{}{path}", api_url());
    let client = if url.starts_with("https://") {
        crate::hbbs_http::create_http_client_async_with_url_strict(&url).await?
    } else {
        crate::hbbs_http::create_http_client_async(TlsType::Plain, false)
    };
    let resp = client
        .post(&url)
        .header("Accept", "application/json")
        .json(body)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await?;
    let status = resp.status().as_u16();
    let text = resp.text().await?;
    let value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok((status, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_is_read_from_the_downloaded_name() {
        let t = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            ticket_from_name(&format!("ERSM-Remoto-Soporte_{t}.exe")).as_deref(),
            Some(t)
        );
        assert_eq!(
            ticket_from_name(&format!("ERSM-Remoto-Soporte_{t} (1).exe")).as_deref(),
            Some(t)
        );
        assert_eq!(ticket_from_name("ERSM-Remoto-Soporte.exe"), None);
        assert_eq!(ticket_from_name("ERSM-Remoto-Soporte_xyz.exe"), None);
        assert_eq!(ticket_from_name("rustdesk.exe"), None);
    }
}
