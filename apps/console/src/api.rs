use reqwest::{Client, Method, Response};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::model::{ApiFailure, AuthResponse, MatchView, RoomList, RoomView, StartMatchResponse};

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: Client,
    token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String) -> Result<Self, ApiFailure> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        let client = Client::builder().build().map_err(transport_error)?;
        Ok(Self {
            base_url,
            client,
            token: None,
        })
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub async fn register(
        &self,
        login_name: &str,
        password: &str,
        nickname: &str,
    ) -> Result<AuthResponse, ApiFailure> {
        self.send(
            Method::POST,
            "/api/v1/registrations",
            Some(json!({
                "login_name": login_name,
                "password": password,
                "nickname": nickname
            })),
        )
        .await
    }

    pub async fn login(
        &self,
        login_name: &str,
        password: &str,
    ) -> Result<AuthResponse, ApiFailure> {
        self.send(
            Method::POST,
            "/api/v1/sessions",
            Some(json!({"login_name": login_name, "password": password})),
        )
        .await
    }

    pub async fn rooms(&self) -> Result<RoomList, ApiFailure> {
        self.send(Method::GET, "/api/v1/rooms", None).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_room(
        &self,
        name: &str,
        variant: &str,
        initial_points: u32,
        tobi: bool,
        noten_payment: u32,
        head_bump: bool,
    ) -> Result<RoomView, ApiFailure> {
        self.send(
            Method::POST,
            "/api/v1/rooms",
            Some(json!({
                "name": name,
                "visibility": "public",
                "rules": {
                    "rule_set_id": format!("riichi/{variant}"),
                    "config": {
                        "overrides": {
                            "match_rules": {
                                "initial_points": initial_points,
                                "tobi": tobi
                            },
                            "settlement": {
                                "noten_payment": noten_payment,
                                "ron_resolution": if head_bump {
                                    "head_bump"
                                } else {
                                    "multiple"
                                }
                            }
                        }
                    }
                }
            })),
        )
        .await
    }

    pub async fn room(&self, room_id: &str) -> Result<RoomView, ApiFailure> {
        self.send(Method::GET, &format!("/api/v1/rooms/{room_id}"), None)
            .await
    }

    pub async fn join_room(
        &self,
        room_id: &str,
        expected_version: u64,
    ) -> Result<RoomView, ApiFailure> {
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
        ready: bool,
    ) -> Result<RoomView, ApiFailure> {
        self.send(
            Method::PUT,
            &format!("/api/v1/rooms/{room_id}/members/me/readiness"),
            Some(json!({"expected_version": expected_version, "ready": ready})),
        )
        .await
    }

    pub async fn leave_room(
        &self,
        room_id: &str,
        expected_version: u64,
    ) -> Result<RoomView, ApiFailure> {
        self.send(
            Method::DELETE,
            &format!("/api/v1/rooms/{room_id}/members"),
            Some(json!({"expected_version": expected_version})),
        )
        .await
    }

    pub async fn start_room(
        &self,
        room_id: &str,
        expected_version: u64,
    ) -> Result<StartMatchResponse, ApiFailure> {
        self.send(
            Method::POST,
            &format!("/api/v1/rooms/{room_id}/matches"),
            Some(json!({"expected_version": expected_version})),
        )
        .await
    }

    pub async fn match_view(&self, match_id: &str) -> Result<MatchView, ApiFailure> {
        self.send(Method::GET, &format!("/api/v1/matches/{match_id}"), None)
            .await
    }

    pub async fn game_command(
        &self,
        match_id: &str,
        expected_version: u64,
        name: &str,
        payload: Option<Value>,
    ) -> Result<MatchView, ApiFailure> {
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
    ) -> Result<T, ApiFailure> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base_url));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        decode(request.send().await.map_err(transport_error)?).await
    }
}

async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, ApiFailure> {
    let status = response.status();
    let value = response.json::<Value>().await.map_err(transport_error)?;
    if !status.is_success() {
        return Err(ApiFailure {
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
    serde_json::from_value(value).map_err(|error| ApiFailure {
        code: "client.invalid_response".to_owned(),
        message: error.to_string(),
    })
}

fn transport_error(error: impl std::fmt::Display) -> ApiFailure {
    ApiFailure {
        code: "client.transport".to_owned(),
        message: error.to_string(),
    }
}
