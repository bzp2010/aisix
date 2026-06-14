use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::{Context, Result};
use http::{HeaderValue, Method, header::HeaderName};
use serde::Deserialize;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::config::etcd;

pub mod defaults {
    use super::*;

    pub fn listen() -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], 3000))
    }

    pub fn admin_listen() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 3001))
    }
}

#[derive(Clone, Debug)]
pub struct AdminKey {
    pub key: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DeploymentAdmin {
    #[serde(default = "Vec::new")]
    pub admin_key: Vec<AdminKey>,
}

impl<'de> Deserialize<'de> for AdminKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            key: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(AdminKey { key: raw.key })
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Deployment {
    #[serde(default)]
    pub etcd: etcd::Config,
    #[serde(default)]
    pub admin: DeploymentAdmin,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ServerCommonTls {
    #[serde(default)]
    pub enabled: bool,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ServerCommonCors {
    #[serde(default)]
    pub enabled: bool,
    pub allowed_origins: Option<Vec<String>>,
    pub allowed_methods: Option<Vec<String>>,
    pub allowed_headers: Option<Vec<String>>,
    pub exposed_headers: Option<Vec<String>>,
    pub allow_credentials: Option<bool>,
}

impl ServerCommonCors {
    pub fn to_cors_layer(&self) -> Result<CorsLayer> {
        let mut cors = CorsLayer::new().allow_credentials(self.allow_credentials.unwrap_or(false));

        if let Some(origins) = self.allowed_origins.as_deref() {
            cors = cors.allow_origin(if origins.iter().any(|o| o == "*") {
                AllowOrigin::any()
            } else {
                AllowOrigin::list(Self::parse_cors_values(
                    "allowed_origin",
                    origins,
                    HeaderValue::from_str,
                )?)
            });
        }

        if let Some(methods) = self.allowed_methods.as_deref() {
            cors = cors.allow_methods(Self::parse_cors_values(
                "allowed_method",
                methods,
                Method::from_str,
            )?);
        }

        if let Some(headers) = self.allowed_headers.as_deref() {
            cors = cors.allow_headers(Self::parse_cors_values(
                "allowed_header",
                headers,
                HeaderName::from_str,
            )?);
        }

        if let Some(headers) = self.exposed_headers.as_deref() {
            cors = cors.expose_headers(Self::parse_cors_values(
                "exposed_header",
                headers,
                HeaderName::from_str,
            )?);
        }

        Ok(cors)
    }

    fn parse_cors_values<T, E, F>(field: &str, values: &[String], mut parse: F) -> Result<Vec<T>>
    where
        F: FnMut(&str) -> std::result::Result<T, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        values
            .iter()
            .map(|value| parse(value).with_context(|| format!("Invalid CORS {}: {}", field, value)))
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerProxy {
    #[serde(default = "defaults::listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub tls: ServerCommonTls,
    #[serde(default)]
    pub cors: ServerCommonCors,
}

impl Default for ServerProxy {
    fn default() -> Self {
        Self {
            listen: defaults::listen(),
            tls: ServerCommonTls::default(),
            cors: ServerCommonCors::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerAdmin {
    #[serde(default = "defaults::admin_listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub tls: ServerCommonTls,
    #[serde(default)]
    pub cors: ServerCommonCors,
}

impl Default for ServerAdmin {
    fn default() -> Self {
        Self {
            listen: defaults::admin_listen(),
            tls: ServerCommonTls::default(),
            cors: ServerCommonCors::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Server {
    #[serde(default)]
    pub proxy: ServerProxy,
    #[serde(default)]
    pub admin: ServerAdmin,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub deployment: Deployment,
    #[serde(default)]
    pub server: Server,
}

#[cfg(test)]
mod tests {
    use super::ServerCommonCors;

    #[test]
    fn to_cors_layer_accepts_valid_config() {
        let cors = ServerCommonCors {
            enabled: true,
            allowed_origins: Some(vec!["https://example.com".into()]),
            allowed_methods: Some(vec!["GET".into(), "POST".into()]),
            allowed_headers: Some(vec!["content-type".into()]),
            exposed_headers: Some(vec!["x-request-id".into()]),
            allow_credentials: Some(true),
        };

        assert!(cors.to_cors_layer().is_ok());
    }

    #[test]
    fn to_cors_layer_rejects_invalid_config() {
        let cors = ServerCommonCors {
            allowed_methods: Some(vec!["NOT A METHOD".into()]),
            ..Default::default()
        };

        let result = cors.to_cors_layer();

        assert!(result.is_err());
        assert!(
            result
                .err()
                .map(|err| err.to_string().contains("Invalid CORS allowed_method"))
                .unwrap_or(false)
        );
    }
}
