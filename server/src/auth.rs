//! X OAuth 2.0 login (authorization code + PKCE) and shop session cookies.

use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use cookie::{Cookie, CookieJar, Key, SameSite};
use maud::html;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use url::Url;

use crate::layout;
use crate::AppState;

const AUTHORIZE_URL: &str = "https://twitter.com/i/oauth2/authorize";
const TOKEN_URL: &str = "https://api.twitter.com/2/oauth2/token";
const ME_URL: &str = "https://api.twitter.com/2/users/me";
const OAUTH_SCOPES: &str = "users.read tweet.read";
const OAUTH_COOKIE: &str = "fossall_oauth";
const SESSION_COOKIE: &str = "fossall_shop";
const OAUTH_MAX_AGE_SECS: i64 = 600;
const SESSION_MAX_AGE_SECS: i64 = 60 * 60 * 24 * 30;
const DEFAULT_NEXT: &str = "/shop";
const DEFAULT_PUBLIC_BASE: &str = "http://localhost:8080";

#[derive(Clone)]
pub struct ShopSettings {
    pub x_client_id: Option<String>,
    pub x_client_secret: Option<String>,
    pub public_base_url: String,
    cookie_key: Option<Key>,
    pub secure_cookies: bool,
    http: reqwest::Client,
}

impl ShopSettings {
    pub fn from_env() -> anyhow::Result<Self> {
        let public_base_url = nonempty_env("PUBLIC_BASE_URL")
            .map(|value| value.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_PUBLIC_BASE.to_string());
        let secure_cookies = public_base_url.starts_with("https://");
        let cookie_key =
            nonempty_env("SHOP_SESSION_SECRET").map(|secret| Key::derive_from(secret.as_bytes()));
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            x_client_id: nonempty_env("X_CLIENT_ID"),
            x_client_secret: nonempty_env("X_CLIENT_SECRET"),
            public_base_url,
            cookie_key,
            secure_cookies,
            http,
        })
    }

    pub fn login_configured(&self) -> bool {
        self.x_client_id.is_some() && self.x_client_secret.is_some() && self.cookie_key.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StartQuery {
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OauthCookie {
    state: String,
    verifier: String,
    next: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct MeResponse {
    data: XProfile,
}

#[derive(Debug, Deserialize)]
struct XProfile {
    id: String,
    name: String,
    username: String,
    profile_image_url: Option<String>,
}

/// `GET /auth/x` — start X login and stash PKCE state in a short-lived cookie.
pub async fn start(State(state): State<AppState>, Query(query): Query<StartQuery>) -> Response {
    let next = sanitize_next(query.next.as_deref());
    let Some((client_id, _)) = client_creds(&state.shop) else {
        return auth_error(
            "Sign-in is not configured on this process. Set X_CLIENT_ID, X_CLIENT_SECRET, and SHOP_SESSION_SECRET (see docs/SHOP.md).",
        );
    };
    let Some(key) = state.shop.cookie_key.as_ref() else {
        return auth_error(
            "Sign-in is not configured on this process. Set SHOP_SESSION_SECRET (see docs/SHOP.md).",
        );
    };

    let state_token = random_token();
    let verifier = random_token();
    let challenge = code_challenge(&verifier);
    let redirect = redirect_uri(&state.shop.public_base_url);
    let authorize = match authorize_url(client_id, &redirect, &state_token, &challenge) {
        Ok(url) => url,
        Err(err) => {
            tracing::error!(?err, "could not build X authorize URL");
            return auth_error("Could not start sign-in with X. Try again shortly.");
        }
    };

    let payload = OauthCookie {
        state: state_token,
        verifier,
        next,
    };
    let Ok(json) = serde_json::to_string(&payload) else {
        return auth_error("Could not start sign-in with X. Try again shortly.");
    };
    let Some(cookie) = private_cookie(
        key,
        OAUTH_COOKIE,
        json,
        OAUTH_MAX_AGE_SECS,
        state.shop.secure_cookies,
    ) else {
        return auth_error("Could not start sign-in with X. Try again shortly.");
    };

    redirect_with_cookies(&authorize, vec![cookie])
}

/// `GET /auth/x/callback` — finish X login and set the shop session cookie.
pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if let Some(error) = query.error.as_deref().filter(|value| !value.is_empty()) {
        tracing::info!(error, "X authorization declined");
        return auth_error(
            "X did not complete sign-in. You can close this page or try again from the shop.",
        );
    }

    let Some((client_id, client_secret)) = client_creds(&state.shop) else {
        return auth_error(
            "Sign-in is not configured on this process. Set X_CLIENT_ID, X_CLIENT_SECRET, and SHOP_SESSION_SECRET (see docs/SHOP.md).",
        );
    };
    let Some(key) = state.shop.cookie_key.as_ref() else {
        return auth_error(
            "Sign-in is not configured on this process. Set SHOP_SESSION_SECRET (see docs/SHOP.md).",
        );
    };
    let Some(pool) = state.db.as_ref() else {
        return auth_error(
            "The shop database is not connected in this process. Set DATABASE_URL (see docs/DATABASE.md) and restart.",
        );
    };

    let Some(stored) = read_oauth_cookie(&headers, key) else {
        return auth_error("Sign-in expired. Start again from the shop.");
    };
    let Some(state_param) = query.state.as_deref() else {
        return auth_error("Sign-in was missing its state parameter. Start again from the shop.");
    };
    if state_param != stored.state {
        tracing::info!("X callback state mismatch");
        return auth_error("Sign-in could not be verified. Start again from the shop.");
    }
    let Some(code) = query.code.as_deref().filter(|value| !value.is_empty()) else {
        return auth_error("X did not return an authorization code. Start again from the shop.");
    };

    let redirect = redirect_uri(&state.shop.public_base_url);
    let profile = match exchange_and_profile(
        &state.shop,
        client_id,
        client_secret,
        code,
        &redirect,
        &stored.verifier,
    )
    .await
    {
        Ok(profile) => profile,
        Err(err) => {
            tracing::error!(?err, "X token or profile exchange failed");
            return auth_error("Could not finish sign-in with X. Try again shortly.");
        }
    };

    let user_id = match upsert_user(pool, &profile).await {
        Ok(id) => id,
        Err(err) => {
            tracing::error!(?err, "shop user upsert failed");
            return auth_error("Could not save the shop session. Try again shortly.");
        }
    };

    let Some(session) = private_cookie(
        key,
        SESSION_COOKIE,
        user_id.to_string(),
        SESSION_MAX_AGE_SECS,
        state.shop.secure_cookies,
    ) else {
        return auth_error("Could not save the shop session. Try again shortly.");
    };
    let mut cookies = Vec::new();
    if let Some(clear) = expire_cookie(OAUTH_COOKIE, state.shop.secure_cookies) {
        cookies.push(clear);
    }
    cookies.push(session);
    redirect_with_cookies(&stored.next, cookies)
}

/// `POST /auth/logout` — drop the shop session cookie.
pub async fn logout(State(state): State<AppState>) -> Response {
    let mut cookies = Vec::new();
    if let Some(clear) = expire_cookie(SESSION_COOKIE, state.shop.secure_cookies) {
        cookies.push(clear);
    }
    if let Some(clear) = expire_cookie(OAUTH_COOKIE, state.shop.secure_cookies) {
        cookies.push(clear);
    }
    redirect_with_cookies(DEFAULT_NEXT, cookies)
}

pub async fn session_user(
    pool: &PgPool,
    settings: &ShopSettings,
    headers: &HeaderMap,
) -> Option<ShopUser> {
    let key = settings.cookie_key.as_ref()?;
    let raw = read_private_cookie(headers, key, SESSION_COOKIE)?;
    let user_id: i64 = raw.parse().ok()?;
    match load_user(pool, user_id).await {
        Ok(user) => user,
        Err(err) => {
            tracing::error!(?err, "shop session lookup failed");
            None
        }
    }
}

pub fn sign_in_href(next: &str) -> String {
    format!("/auth/x?next={next}")
}

pub fn sanitize_next(raw: Option<&str>) -> String {
    let Some(value) = raw.filter(|value| !value.is_empty()) else {
        return DEFAULT_NEXT.to_string();
    };
    if value.contains("..")
        || value.contains('\\')
        || value.contains('\n')
        || value.contains('\r')
        || value.contains('\0')
        || value.contains(':')
        || !value.starts_with('/')
    {
        return DEFAULT_NEXT.to_string();
    }
    if value == "/shop" || value.starts_with("/shop/") || value.starts_with("/shop?") {
        value.to_string()
    } else {
        DEFAULT_NEXT.to_string()
    }
}

pub fn redirect_uri(public_base_url: &str) -> String {
    format!("{}/auth/x/callback", public_base_url.trim_end_matches('/'))
}

pub fn authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> Result<String, url::ParseError> {
    let mut url = Url::parse(AUTHORIZE_URL)?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", OAUTH_SCOPES)
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url.into())
}

pub fn code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

fn client_creds(settings: &ShopSettings) -> Option<(&str, &str)> {
    Some((
        settings.x_client_id.as_deref()?,
        settings.x_client_secret.as_deref()?,
    ))
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

async fn exchange_and_profile(
    settings: &ShopSettings,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<XProfile, anyhow::Error> {
    let token_res = settings
        .http
        .post(TOKEN_URL)
        .basic_auth(client_id, Some(client_secret))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ])
        .send()
        .await?;
    let token_status = token_res.status();
    if !token_status.is_success() {
        anyhow::bail!("x token status {token_status}");
    }
    let token: TokenResponse = token_res.json().await?;
    let me_res = settings
        .http
        .get(ME_URL)
        .query(&[("user.fields", "id,name,username,profile_image_url")])
        .bearer_auth(&token.access_token)
        .send()
        .await?;
    let me_status = me_res.status();
    if !me_status.is_success() {
        anyhow::bail!("x users/me status {me_status}");
    }
    let me: MeResponse = me_res.json().await?;
    if me.data.id.is_empty() || me.data.username.is_empty() {
        anyhow::bail!("x users/me missing id or username");
    }
    Ok(me.data)
}

async fn upsert_user(pool: &PgPool, profile: &XProfile) -> Result<i64, sqlx::Error> {
    let username = truncate(&profile.username, 64);
    let display_name = truncate(&profile.name, 80);
    let avatar = profile.avatar_url().map(|url| truncate(url, 512));
    sqlx::query_scalar(
        "INSERT INTO shop.users (x_user_id, username, display_name, avatar_url)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (x_user_id) DO UPDATE SET
            username = EXCLUDED.username,
            display_name = EXCLUDED.display_name,
            avatar_url = EXCLUDED.avatar_url,
            updated_at = now()
         RETURNING id",
    )
    .bind(&profile.id)
    .bind(username)
    .bind(display_name)
    .bind(avatar)
    .fetch_one(pool)
    .await
}

impl XProfile {
    fn avatar_url(&self) -> Option<&str> {
        self.profile_image_url
            .as_deref()
            .filter(|url| !url.is_empty())
    }
}

async fn load_user(pool: &PgPool, user_id: i64) -> Result<Option<ShopUser>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, username, display_name, avatar_url
         FROM shop.users
         WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| ShopUser {
        id: row.get("id"),
        username: row.get("username"),
        display_name: row.get("display_name"),
        avatar_url: row.get("avatar_url"),
    }))
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn private_cookie(
    key: &Key,
    name: &'static str,
    value: String,
    max_age_secs: i64,
    secure: bool,
) -> Option<HeaderValue> {
    let mut jar = CookieJar::new();
    let cookie = Cookie::build((name, value))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .path("/")
        .max_age(cookie::time::Duration::seconds(max_age_secs))
        .build();
    jar.private_mut(key).add(cookie);
    let encoded = jar.delta().next()?;
    HeaderValue::from_str(&encoded.to_string()).ok()
}

fn expire_cookie(name: &'static str, secure: bool) -> Option<HeaderValue> {
    let cookie = Cookie::build((name, ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .path("/")
        .max_age(cookie::time::Duration::ZERO)
        .build();
    HeaderValue::from_str(&cookie.to_string()).ok()
}

fn read_oauth_cookie(headers: &HeaderMap, key: &Key) -> Option<OauthCookie> {
    let raw = read_private_cookie(headers, key, OAUTH_COOKIE)?;
    serde_json::from_str(&raw).ok()
}

fn read_private_cookie(headers: &HeaderMap, key: &Key, name: &str) -> Option<String> {
    let header = headers.get(header::COOKIE)?.to_str().ok()?;
    let mut jar = CookieJar::new();
    for cookie in Cookie::split_parse(header.to_string()) {
        let Ok(cookie) = cookie else {
            continue;
        };
        jar.add_original(cookie);
    }
    jar.private(key)
        .get(name)
        .map(|cookie| cookie.value().to_string())
}

fn redirect_with_cookies(location: &str, cookies: Vec<HeaderValue>) -> Response {
    let mut response = Redirect::to(location).into_response();
    for cookie in cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

fn auth_error(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        layout::page(
            "Sign-in",
            html! {
                section class="shop" {
                    p class="shop-banner" role="status" {
                        strong { "Simulation." }
                        " Nothing is charged or shipped."
                    }
                    header class="essay-header" {
                        p class="eyebrow" { "X login" }
                        h1 { "Could not sign in" }
                    }
                    p class="words-error" { (message) }
                    p {
                        a class="btn secondary" href="/shop" { "Back to the shop" }
                    }
                }
            },
        ),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn sanitize_next_allows_shop_paths_only() {
        assert_eq!(sanitize_next(None), "/shop");
        assert_eq!(sanitize_next(Some("/shop")), "/shop");
        assert_eq!(sanitize_next(Some("/shop/cart")), "/shop/cart");
        assert_eq!(sanitize_next(Some("/shop/orders/3")), "/shop/orders/3");
        assert_eq!(sanitize_next(Some("/shop?from=home")), "/shop?from=home");
    }

    #[test]
    fn sanitize_next_rejects_open_redirects() {
        assert_eq!(sanitize_next(Some("https://evil.example")), "/shop");
        assert_eq!(sanitize_next(Some("//evil.example")), "/shop");
        assert_eq!(sanitize_next(Some("/words")), "/shop");
        assert_eq!(sanitize_next(Some("/shop/../auth")), "/shop");
        assert_eq!(sanitize_next(Some("/shopping")), "/shop");
    }

    #[test]
    fn code_challenge_matches_rfc_7636_appendix_b() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_uses_pkce_s256_and_read_scopes() {
        let url = authorize_url(
            "client123",
            "http://localhost:8080/auth/x/callback",
            "state-value",
            "challenge-value",
        )
        .unwrap();
        let parsed = Url::parse(&url).unwrap();
        assert_eq!(parsed.host_str(), Some("twitter.com"));
        assert_eq!(parsed.path(), "/i/oauth2/authorize");
        let pairs: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            pairs.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            pairs.get("client_id").map(String::as_str),
            Some("client123")
        );
        assert_eq!(
            pairs.get("code_challenge").map(String::as_str),
            Some("challenge-value")
        );
        assert!(!pairs.contains_key("access_token"));
        assert!(!pairs.contains_key("client_secret"));
        let scope = pairs.get("scope").expect("scope");
        assert!(scope.contains("users.read"));
        assert!(scope.contains("tweet.read"));
        assert!(!scope.contains("offline.access"));
    }

    #[test]
    fn redirect_uri_strips_trailing_slash() {
        assert_eq!(
            redirect_uri("https://fossall.com/"),
            "https://fossall.com/auth/x/callback"
        );
    }
}
