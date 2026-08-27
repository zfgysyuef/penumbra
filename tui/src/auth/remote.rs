/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2026 Shomy
*/
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, bail};
use log::info;
use penumbra::error::AuthError;
use penumbra::{SignPurpose, SignRequest, Signer, VERSION};
use serde::{Deserialize, Serialize};
use ureq::http::Response;
use ureq::typestate::{WithBody, WithoutBody};
use ureq::{Body, RequestBuilder};
use url::Url;

use crate::config::AntumbraConfig;

#[derive(Deserialize, Serialize, Debug, Clone)]
struct AuthState {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

pub struct RemoteSigner {
    config: Arc<AntumbraConfig>,
}

#[derive(Serialize)]
struct ApiSignData {
    rnd: String,
    soc_id: String,
    hrid: String,
    raw: String,
}

#[derive(Serialize)]
struct ApiSignRequest {
    data: ApiSignData,
    purpose: String,
    pubk_mod: String,
}

#[derive(Deserialize)]
struct ApiSignResponse {
    signature: String,
}

#[derive(Serialize)]
struct CanSignRequest {
    pubk_mod: String,
}

#[derive(Deserialize)]
struct CanSignResponse {
    can_sign: bool,
}

#[derive(Deserialize)]
struct AuthorizeResponse {
    authorized: bool,
}

#[derive(Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct UserMeResponse {
    is_admin: bool,
    permissions: Vec<String>,
}

impl RemoteSigner {
    pub const fn new(config: Arc<AntumbraConfig>) -> Self {
        Self { config }
    }

    fn get_config_path() -> Option<PathBuf> {
        let dir = dirs::config_dir()?.join("antumbra");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        Some(dir.join("auth.json"))
    }

    fn load_auth_state() -> Option<AuthState> {
        let path = Self::get_config_path()?;
        if path.exists()
            && let Ok(data) = fs::read_to_string(path)
            && let Ok(state) = serde_json::from_str::<AuthState>(&data)
        {
            return Some(state);
        }
        None
    }

    fn save_auth_state(state: &AuthState) -> Result<()> {
        let path =
            Self::get_config_path().ok_or_else(|| anyhow::anyhow!("Config directory not found"))?;
        let data = serde_json::to_string_pretty(state)?;
        fs::write(path, data)?;
        Ok(())
    }

    const fn purpose_to_str(purpose: SignPurpose) -> &'static str {
        match purpose {
            SignPurpose::BromSla => "brom_sla",
            SignPurpose::PlSla => "pl_sla",
            SignPurpose::MetaSla => "meta_sla",
            SignPurpose::DaSla => "da_sla",
        }
    }

    fn get_valid_endpoint(&self) -> Option<&str> {
        let endpoint = self.config.auth.endpoint.as_deref()?;
        if !self.config.auth.online_auth || Url::parse(endpoint).is_err() {
            return None;
        }
        Some(endpoint)
    }

    fn get(&self, path: &str, token: Option<&str>) -> Result<RequestBuilder<WithoutBody>> {
        let Some(endpoint) = self.get_valid_endpoint() else {
            bail!("Invalid endpoint");
        };
        let url = format!("{}{}", endpoint, path);

        let mut req = ureq::get(&url);

        req = req.header("User-Agent", &format!("Antumbra (v{VERSION})"));

        if let Some(t) = token {
            req = req.header("Authorization", &format!("Bearer {}", t));
            req = req.header("Accept", "application/json");
            req = req.header("Content-Type", "application/json");
        }

        Ok(req)
    }

    fn post(&self, path: &str, token: Option<&str>) -> Result<RequestBuilder<WithBody>> {
        let Some(endpoint) = self.get_valid_endpoint() else {
            bail!("Invalid endpoint");
        };

        let url = format!("{}{}", endpoint, path);
        let mut req = ureq::post(&url);

        req = req.header("User-Agent", &format!("Antumbra (v{VERSION})"));

        if let Some(t) = token {
            req = req.header("Authorization", &format!("Bearer {}", t));
        }

        Ok(req)
    }

    fn post_json<B: Serialize>(
        &self,
        path: &str,
        token: Option<&str>,
        body: &B,
    ) -> Result<Response<Body>> {
        let req = self.post(path, token)?;
        let res = req.send_json(body)?;
        Ok(res)
    }

    fn fetch_user_info(&self, access_token: &str) -> Option<UserMeResponse> {
        let mut res = self.get("/api/auth/me", Some(access_token)).ok()?.call().ok()?;
        if res.status().is_success() {
            res.body_mut().read_json::<UserMeResponse>().ok()
        } else {
            None
        }
    }

    fn refresh_auth_token(&self, refresh_token: &str) -> Option<AuthState> {
        let mut res =
            self.post("/api/auth/refresh", Some(refresh_token)).ok()?.send_empty().ok()?;
        if res.status().is_success()
            && let Ok(tokens) = res.body_mut().read_json::<TokenResponse>()
        {
            return Some(AuthState {
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                expires_in: tokens.expires_in,
            });
        }
        None
    }

    fn login_with_credentials(&self, username: &str, password: &str) -> Option<AuthState> {
        let req_body = LoginRequest { username, password };
        let mut res = self.post_json("/api/auth/login", None, &req_body).unwrap();

        if res.status().is_success()
            && let Ok(tokens) = res.body_mut().read_json::<TokenResponse>()
        {
            return Some(AuthState {
                access_token: tokens.access_token,
                refresh_token: tokens.refresh_token,
                expires_in: tokens.expires_in,
            });
        }
        None
    }

    fn ensure_authenticated(&self) -> Option<String> {
        if let Some(state) = Self::load_auth_state() {
            if self.fetch_user_info(&state.access_token).is_some() {
                return Some(state.access_token);
            }

            if let Some(new_state) = self.refresh_auth_token(&state.refresh_token) {
                Self::save_auth_state(&new_state).ok()?;
                return Some(new_state.access_token);
            }
        }

        if let (Some(username), Some(password)) =
            (&self.config.auth.username, &self.config.auth.password)
            && let Some(new_state) = self.login_with_credentials(username, password)
        {
            Self::save_auth_state(&new_state).ok()?;
            return Some(new_state.access_token);
        }

        None
    }
}

impl Signer for RemoteSigner {
    fn can_handle(&self, pubk_mod: &[u8]) -> bool {
        let Some(access_token) = self.ensure_authenticated() else {
            return false;
        };

        let req_body = CanSignRequest { pubk_mod: hex::encode(pubk_mod) };

        match self.post_json("/api/v1/can-sign", Some(&access_token), &req_body) {
            Ok(mut res) if res.status().is_success() => {
                res.body_mut().read_json::<CanSignResponse>().map(|r| r.can_sign).unwrap_or(false)
            }
            _ => false,
        }
    }

    fn is_authorized(&self, req: &SignRequest) -> bool {
        let Some(access_token) = self.ensure_authenticated() else {
            return false;
        };

        let Some(user_info) = self.fetch_user_info(&access_token) else {
            return false;
        };

        if user_info.is_admin {
            return true;
        }

        let req_body = ApiSignRequest {
            data: ApiSignData {
                rnd: hex::encode(&req.data.rnd),
                soc_id: hex::encode(&req.data.soc_id),
                hrid: hex::encode(&req.data.hrid),
                raw: hex::encode(&req.data.raw),
            },
            purpose: Self::purpose_to_str(req.purpose).to_string(),
            pubk_mod: hex::encode(&req.pubk_mod),
        };

        let authorized =
            match self.post_json("/api/v1/is-authorized", Some(&access_token), &req_body) {
                Ok(mut res) if res.status().is_success() => res
                    .body_mut()
                    .read_json::<AuthorizeResponse>()
                    .map(|r| r.authorized)
                    .unwrap_or(false),
                _ => false,
            };

        let required_perm = Self::purpose_to_str(req.purpose);
        user_info.permissions.iter().any(|p| p == required_perm) && authorized
    }

    fn sign(&self, req: &SignRequest) -> penumbra::Result<Vec<u8>> {
        let access_token = self.ensure_authenticated().ok_or_else(|| {
            penumbra::Error::Auth(AuthError::Other("Authentication failed".into()))
        })?;

        let req_body = ApiSignRequest {
            data: ApiSignData {
                rnd: hex::encode(&req.data.rnd),
                soc_id: hex::encode(&req.data.soc_id),
                hrid: hex::encode(&req.data.hrid),
                raw: hex::encode(&req.data.raw),
            },
            purpose: Self::purpose_to_str(req.purpose).to_string(),
            pubk_mod: hex::encode(&req.pubk_mod),
        };

        info!("Signing {:?} with remote server...", req.purpose);

        let mut response =
            self.post_json("/api/v1/sign", Some(&access_token), &req_body).map_err(|e| {
                penumbra::Error::Auth(AuthError::Other(format!("Failed to send sign request: {e}")))
            })?;

        let sign_res: ApiSignResponse = response.body_mut().read_json().map_err(|e| {
            penumbra::Error::Auth(AuthError::Other(format!("Failed to parse sign response: {e}")))
        })?;

        info!("Received signature from remote server (0x{:X} bytes)", sign_res.signature.len());

        hex::decode(&sign_res.signature).map_err(|e| {
            penumbra::Error::Auth(AuthError::Other(format!("Failed to decode signature: {e}")))
        })
    }
}
