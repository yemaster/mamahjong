use std::error::Error;
use std::fmt::{self, Display, Formatter};

use reqwest::{Client, Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;

use crate::model::{AuthResponse, MatchView, RoomView, StartMatchResponse};
use crate::runner::Variant;

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: Client,
    token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Result<Self, ApiError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(ApiError::transport)?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            client,
            token: None,
        })
    }

    pub async fn register(
        &mut self,
        login_name: &str,
        nickname: &str,
    ) -> Result<AuthResponse, ApiError> {
        let response: AuthResponse = self
            .send(
                Method::POST,
                "/api/v1/registrations",
                Some(json!({
                    "login_name": login_name,
                    "password": "mamahjong bot password",
                    "nickname": nickname
                })),
            )
            .await?;
        self.token = Some(response.session.token.clone());
        Ok(response)
    }

    pub async fn create_room(&self, name: &str, variant: Variant) -> Result<RoomView, ApiError> {
        self.send(
            Method::POST,
            "/api/v1/rooms",
            Some(json!({
                "name": name,
                "visibility": "private",
                "rules": {
                    "rule_set_id": variant.rule_set_id(),
                    "config": {
                        "overrides": {
                            "match_rules": {
                                "length": "east_only",
                                "tobi": false,
                                "agari_yame": true
                            }
                        }
                    }
                }
            })),
        )
        .await
    }

    pub async fn join_room(
        &self,
        room_id: &str,
        expected_version: u64,
    ) -> Result<RoomView, ApiError> {
        self.send(
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/members"),
            Some(json!({"expected_version": expected_version})),
        )
        .await
    }

    pub async fn set_ready(
        &self,
        room_id: &str,
        expected_version: u64,
    ) -> Result<RoomView, ApiError> {
        self.send(
            Method::PUT,
            &format!("/api/v1/rooms/{room_id}/members/me/readiness"),
            Some(json!({
                "expected_version": expected_version,
                "ready": true
            })),
        )
        .await
    }

    pub async fn start_room(
        &self,
        room_id: &str,
        expected_version: u64,
    ) -> Result<StartMatchResponse, ApiError> {
        self.send(
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/matches"),
            Some(json!({"expected_version": expected_version})),
        )
        .await
    }

    pub async fn match_view(&self, match_id: &str) -> Result<MatchView, ApiError> {
        self.send(Method::GET, &format!("/api/v1/matches/{match_id}"), None)
            .await
    }

    pub async fn game_command(
        &self,
        match_id: &str,
        expected_version: u64,
        name: &str,
        payload: Option<Value>,
    ) -> Result<MatchView, ApiError> {
        let command = match payload {
            Some(payload) => json!({"name": name, "payload": payload}),
            None => json!({"name": name}),
        };
        self.send(
            Method::POST,
            &format!("/api/v1/matches/{match_id}/commands"),
            Some(json!({
                "expected_version": expected_version,
                "command": command
            })),
        )
        .await
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, ApiError> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base_url));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        decode(request.send().await.map_err(ApiError::transport)?).await
    }
}

async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    let status = response.status();
    let value = response
        .json::<Value>()
        .await
        .map_err(ApiError::transport)?;
    if !status.is_success() {
        return Err(ApiError {
            status,
            code: value["code"]
                .as_str()
                .unwrap_or("server.unknown")
                .to_owned(),
            message: value["message"]
                .as_str()
                .unwrap_or("unknown server error")
                .to_owned(),
        });
    }
    serde_json::from_value(value).map_err(|error| ApiError {
        status,
        code: "client.invalid_response".to_owned(),
        message: error.to_string(),
    })
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl ApiError {
    fn transport(error: impl Display) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "client.transport".to_owned(),
            message: error.to_string(),
        }
    }

    pub fn is_invalid_command(&self) -> bool {
        self.code == "game.invalid_command"
    }
}

impl Display for ApiError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({}): {}",
            self.code, self.status, self.message
        )
    }
}

impl Error for ApiError {}
