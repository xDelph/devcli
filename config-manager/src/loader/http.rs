//! HTTP configuration loader implementation.

use crate::core::{Error, Result};
use crate::loader::AsyncConfigLoader;
use async_trait::async_trait;
use reqwest::{header, Client};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

/// Authentication method for HTTP requests.
#[derive(Debug, Clone)]
pub enum HttpAuth {
    /// No authentication
    None,
    /// Bearer token authentication
    Bearer(String),
    /// Basic authentication (username, password)
    Basic(String, String),
}

/// HTTP-based configuration loader.
///
/// Fetches configuration from HTTP/HTTPS endpoints.
///
/// # Examples
///
/// ```rust,no_run
/// use config_manager::loader::{HttpLoader, AsyncConfigLoader, HttpAuth};
/// use serde::{Deserialize, Serialize};
/// use std::time::Duration;
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
/// }
///
/// #[tokio::main]
/// async fn main() -> config_manager::core::Result<()> {
///     let loader = HttpLoader::new("https://api.example.com/config")
///         .with_auth(HttpAuth::Bearer("token123".to_string()))
///         .with_timeout(Duration::from_secs(30));
///
///     let config: MyConfig = loader.load().await?;
///     println!("Loaded: {}", config.name);
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HttpLoader {
    url: String,
    auth: HttpAuth,
    timeout: Duration,
    user_agent: String,
}

impl HttpLoader {
    /// Create a new HTTP loader for the specified URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The HTTP/HTTPS URL to fetch configuration from
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            auth: HttpAuth::None,
            timeout: Duration::from_secs(30),
            user_agent: format!("config-manager/{}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Set the authentication method.
    ///
    /// Default: None
    pub fn with_auth(mut self, auth: HttpAuth) -> Self {
        self.auth = auth;
        self
    }

    /// Set the request timeout.
    ///
    /// Default: 30 seconds
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set a custom User-Agent header.
    ///
    /// Default: "config-manager/{version}"
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Build the HTTP client with configured settings.
    fn build_client(&self) -> reqwest::Result<Client> {
        Client::builder()
            .timeout(self.timeout)
            .user_agent(&self.user_agent)
            .build()
    }
}

#[async_trait]
impl<T> AsyncConfigLoader<T> for HttpLoader
where
    T: DeserializeOwned + Serialize + Send + Sync,
{
    async fn load(&self) -> Result<T> {
        let client = self
            .build_client()
            .map_err(|e| Error::LoadError(format!("Failed to create HTTP client: {}", e)))?;

        let mut request = client.get(&self.url);

        // Add authentication
        match &self.auth {
            HttpAuth::None => {}
            HttpAuth::Bearer(token) => {
                request = request.header(header::AUTHORIZATION, format!("Bearer {}", token));
            }
            HttpAuth::Basic(username, password) => {
                request = request.basic_auth(username, Some(password));
            }
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| Error::LoadError(format!("HTTP request failed: {}", e)))?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            return Err(Error::LoadError(format!(
                "HTTP request failed with status {}: {}",
                status,
                response.text().await.unwrap_or_default()
            )));
        }

        // Parse JSON response
        response
            .json::<T>()
            .await
            .map_err(|e| Error::LoadError(format!("Failed to parse JSON response: {}", e)))
    }

    async fn save(&self, config: &T) -> Result<()> {
        let client = self
            .build_client()
            .map_err(|e| Error::SaveError(format!("Failed to create HTTP client: {}", e)))?;

        let mut request = client.put(&self.url).json(config);

        // Add authentication
        match &self.auth {
            HttpAuth::None => {}
            HttpAuth::Bearer(token) => {
                request = request.header(header::AUTHORIZATION, format!("Bearer {}", token));
            }
            HttpAuth::Basic(username, password) => {
                request = request.basic_auth(username, Some(password));
            }
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| Error::SaveError(format!("HTTP request failed: {}", e)))?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            return Err(Error::SaveError(format!(
                "HTTP request failed with status {}: {}",
                status,
                response.text().await.unwrap_or_default()
            )));
        }

        Ok(())
    }

    fn exists(&self) -> bool {
        // For HTTP endpoints, we can't easily check existence synchronously
        // Return true by default - actual existence is checked during load
        true
    }

    fn source_info(&self) -> String {
        self.url.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        name: String,
        value: u32,
    }

    #[test]
    fn test_http_loader_creation() {
        let loader = HttpLoader::new("https://api.example.com/config")
            .with_auth(HttpAuth::Bearer("token".to_string()))
            .with_timeout(Duration::from_secs(10));

        assert_eq!(loader.url, "https://api.example.com/config");
        assert_eq!(loader.timeout, Duration::from_secs(10));
        assert!(matches!(loader.auth, HttpAuth::Bearer(_)));
    }

    #[test]
    fn test_http_loader_source_info() {
        let loader = HttpLoader::new("https://example.com/config.json");
        assert_eq!(
            AsyncConfigLoader::<TestConfig>::source_info(&loader),
            "https://example.com/config.json"
        );
    }

    // Integration tests require a real HTTP server
    // These would typically use a mock server like wiremock
}
