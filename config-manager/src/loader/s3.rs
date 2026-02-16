//! AWS S3 configuration loader implementation.

use crate::core::{Error, Result};
use crate::loader::AsyncConfigLoader;
use async_trait::async_trait;
use aws_sdk_s3::{config::Region, primitives::ByteStream, Client};
use serde::{de::DeserializeOwned, Serialize};

/// AWS S3-based configuration loader.
///
/// Loads and saves configuration from/to AWS S3 buckets.
///
/// # Authentication
///
/// Uses AWS SDK's default credential chain:
/// - Environment variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
/// - AWS credentials file (~/.aws/credentials)
/// - IAM instance profile (when running on EC2)
/// - ECS task role (when running on ECS)
///
/// # Examples
///
/// ```rust,no_run
/// use config_manager::loader::{S3Loader, AsyncConfigLoader};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// #[tokio::main]
/// async fn main() -> config_manager::core::Result<()> {
///     // Load from S3
///     let loader = S3Loader::new("my-bucket", "config/app.json")
///         .with_region("us-east-1");
///
///     let config: MyConfig = loader.load().await?;
///     println!("Loaded: {}", config.name);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct S3Loader {
    bucket: String,
    key: String,
    region: Option<String>,
}

impl S3Loader {
    /// Create a new S3 loader.
    ///
    /// # Arguments
    ///
    /// * `bucket` - S3 bucket name
    /// * `key` - S3 object key (path to the config file)
    pub fn new(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            key: key.into(),
            region: None,
        }
    }

    /// Set the AWS region.
    ///
    /// If not set, uses the default region from AWS config or environment.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Create an S3 client with the configured settings.
    async fn create_client(&self) -> Result<Client> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());

        if let Some(ref region) = self.region {
            config_loader = config_loader.region(Region::new(region.clone()));
        }

        let config = config_loader.load().await;
        Ok(Client::new(&config))
    }
}

#[async_trait]
impl<T> AsyncConfigLoader<T> for S3Loader
where
    T: DeserializeOwned + Serialize + Send + Sync,
{
    async fn load(&self) -> Result<T> {
        let client = self.create_client().await?;

        // Get object from S3
        let response = client
            .get_object()
            .bucket(&self.bucket)
            .key(&self.key)
            .send()
            .await
            .map_err(|e| {
                Error::LoadError(format!(
                    "Failed to get object from S3 bucket '{}' key '{}': {}",
                    self.bucket, self.key, e
                ))
            })?;

        // Read body
        let data = response
            .body
            .collect()
            .await
            .map_err(|e| Error::LoadError(format!("Failed to read S3 object body: {}", e)))?;

        let bytes = data.into_bytes();

        // Parse JSON
        serde_json::from_slice(&bytes)
            .map_err(|e| Error::LoadError(format!("Failed to parse JSON from S3 object: {}", e)))
    }

    async fn save(&self, config: &T) -> Result<()> {
        let client = self.create_client().await?;

        // Serialize config to JSON
        let json_bytes = serde_json::to_vec_pretty(config)
            .map_err(|e| Error::SaveError(format!("Failed to serialize config: {}", e)))?;

        // Upload to S3
        client
            .put_object()
            .bucket(&self.bucket)
            .key(&self.key)
            .body(ByteStream::from(json_bytes))
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| {
                Error::SaveError(format!(
                    "Failed to put object to S3 bucket '{}' key '{}': {}",
                    self.bucket, self.key, e
                ))
            })?;

        Ok(())
    }

    fn exists(&self) -> bool {
        // For S3, we can't easily check existence synchronously
        // Return true by default - actual existence is checked during load
        true
    }

    fn source_info(&self) -> String {
        format!("s3://{}/{}", self.bucket, self.key)
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
    fn test_s3_loader_creation() {
        let loader = S3Loader::new("my-bucket", "config/app.json").with_region("us-west-2");

        assert_eq!(loader.bucket, "my-bucket");
        assert_eq!(loader.key, "config/app.json");
        assert_eq!(loader.region, Some("us-west-2".to_string()));
    }

    #[test]
    fn test_s3_loader_source_info() {
        let loader = S3Loader::new("test-bucket", "configs/test.json");
        assert_eq!(
            AsyncConfigLoader::<TestConfig>::source_info(&loader),
            "s3://test-bucket/configs/test.json"
        );
    }

    // Integration tests would require actual S3 access or LocalStack
    // These are typically run in CI with proper AWS credentials
}
