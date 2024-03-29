use lazy_static::lazy_static;
use serde::Serialize;
use std::{borrow::Cow, env};
use unhtml::{self, FromHtml};
use url::{self, Url};
use warp::{Filter, Reply};

lazy_static! {
    static ref MODEM_IP: Cow<'static, str> =
        env::var("MODEM_IP").map_or(Cow::Borrowed("192.168.100.1"), Cow::Owned);
    static ref MODEM_USER: Cow<'static, str> =
        env::var("MODEM_USER").map_or(Cow::Borrowed("admin"), Cow::Owned);
    static ref MODEM_PASSWORD: Option<String> = env::var("MODEM_PASSWORD").ok();
    static ref AUTH_URL: Option<Result<Url, url::ParseError>> =
        MODEM_PASSWORD.as_ref().map(|password| Url::parse(&format!(
            "https://{}/cmconnectionstatus.html?login_{}",
            MODEM_IP.as_ref(),
            base64::encode(format!("{}:{}", MODEM_USER.as_ref(), password))
        )));
    static ref STATUS_URL: Result<Url, url::ParseError> = Url::parse(&format!(
        "https://{}/cmconnectionstatus.html",
        MODEM_IP.as_ref()
    ));
    static ref LOGOUT_URL: Result<Url, url::ParseError> = Url::parse(&format!(
        "https://{}/logout.html",
        MODEM_IP.as_ref()
    ));
}

#[derive(FromHtml, Serialize)]
#[html(selector = ".content table:nth-of-type(1)")]
struct StartupProcedure {
    #[html(selector = "tr:nth-of-type(3)")]
    acquire_downstream_channel: Status,
    #[html(selector = "tr:nth-of-type(4)")]
    connectivity_state: Status,
    #[html(selector = "tr:nth-of-type(5)")]
    boot_state: Status,
    #[html(selector = "tr:nth-of-type(6)")]
    configuration_file: Status,
    #[html(selector = "tr:nth-of-type(7)")]
    security: Status,
    #[html(selector = "tr:nth-of-type(8)")]
    docsis_network_enabled: Status,
}

#[derive(FromHtml, Serialize)]
struct Status {
    #[html(selector = "td:nth-of-type(2)", attr = "inner")]
    status: String,
    #[html(selector = "td:nth-of-type(3)", attr = "inner", default = "")]
    comment: String,
}

async fn get_token(client: &reqwest::Client) -> Result<String, warp::reply::WithStatus<String>> {
    match client
        .get(AUTH_URL.as_ref().unwrap().as_ref().unwrap().clone())
        .basic_auth(MODEM_USER.as_ref(), Some(MODEM_PASSWORD.as_ref().unwrap()))
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded; charset=utf-8",
        )
        .send()
        .await
    {
        Ok(result) if result.status().is_success() => match result.text().await {
            Ok(credential) => Ok(credential),
            Err(error) => Err(warp::reply::with_status(
                format!("Bad data: {:?}", error),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            )),
        },
        Ok(result) => Err(warp::reply::with_status(
            format!("Server error: {:?}", result),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        )),
        Err(error) => Err(warp::reply::with_status(
            format!("Network error: {:?}", error),
            warp::http::StatusCode::BAD_GATEWAY,
        )),
    }
}

async fn test(
    client: &reqwest::Client,
) -> Result<warp::reply::Json, warp::reply::WithStatus<String>> {
    let token = match get_token(client).await {
        Ok(token) => token,
        Err(error) => return Err(error),
    };
    let mut status_url = STATUS_URL.as_ref().unwrap().clone();
    status_url.set_query(Some(&format!("ct_{}", token)));
    let request = client
        .get(status_url)
        .build()
        .unwrap();
    match client.execute(request).await {
        Ok(result) if result.status().is_success() => match result.text().await {
            Ok(text) => match StartupProcedure::from_html(&text) {
                Ok(status) => {
                    Ok(warp::reply::json(&status))
                },
                Err(error) => Err(warp::reply::with_status(
                    format!("No status found: {:?}", error),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                )),
            },
            Err(error) => Err(warp::reply::with_status(
                format!("Bad data: {:?}", error),
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            )),
        },
        Ok(result) => Err(warp::reply::with_status(
            format!("Server error: {:?}", result),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        )),
        Err(error) => Err(warp::reply::with_status(
            format!("Network error: {:?}", error),
            warp::http::StatusCode::BAD_GATEWAY,
        )),
    }
}

async fn test_handler() -> Result<warp::reply::Response, warp::reject::Rejection> {    
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    // Sometimes the modem sends the login page instead of the status page.
    // Just hammer the thing with requests until it sends the correct response.
    let mut tries = 15;
    Ok(loop {
        break match test(&client).await {
            Ok(r) => r.into_response(),
            Err(ref e) if tries > 0 => {
                eprintln!("trying {} more times: {:?}", tries, e);
                tries -= 1;
                continue
            },
            Err(e) => e.into_response(),
        }
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let health = warp::path!("health").and_then(test_handler);

    warp::serve(health).run(([0, 0, 0, 0], 3030)).await;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn select() {
        let text = include_str!("ex.html");

        let result = StartupProcedure::from_html(&text).unwrap();
        assert_eq!("675000000 Hz", result.acquire_downstream_channel.status);
        assert_eq!("Locked", result.acquire_downstream_channel.comment);
        assert_eq!("OK", result.connectivity_state.status);
        assert_eq!("Operational", result.connectivity_state.comment);
        assert_eq!("OK", result.boot_state.status);
        assert_eq!("Operational", result.boot_state.comment);
        assert_eq!("OK", result.configuration_file.status);
        assert_eq!("", result.configuration_file.comment);
        assert_eq!("Enabled", result.security.status);
        assert_eq!("BPI+", result.security.comment);
        assert_eq!("Allowed", result.docsis_network_enabled.status);
        assert_eq!("", result.docsis_network_enabled.comment);
    }
}
