use serde::Deserialize;

/// Typed application configuration loaded from environment variables.
/// Uses the `config` crate for robust multi-source configuration support.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub encryption_key: String,
    pub alpaca: AlpacaConfig,
    pub finnhub: FinnhubConfig,
    #[serde(default = "default_server_config")]
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AlpacaConfig {
    pub api_key: String,
    pub api_secret: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FinnhubConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_max_db_connections")]
    pub max_db_connections: u32,
}

fn default_server_config() -> ServerConfig {
    ServerConfig {
        host: default_host(),
        port: default_port(),
        max_db_connections: default_max_db_connections(),
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_max_db_connections() -> u32 {
    5
}

impl AppConfig {
    /// Load configuration from environment variables.
    /// Mapping:
    ///   DATABASE_URL          -> database_url
    ///   JWT_SECRET            -> jwt_secret
    ///   ENCRYPTION_KEY        -> encryption_key
    ///   ALPACA_API_KEY        -> alpaca.api_key
    ///   ALPACA_API_SECRET     -> alpaca.api_secret
    ///   FINNHUB_API_KEY       -> finnhub.api_key
    ///   SERVER_HOST           -> server.host
    ///   SERVER_PORT           -> server.port
    ///   MAX_DB_CONNECTIONS    -> server.max_db_connections
    pub fn from_env() -> anyhow::Result<Self> {
        let config = AppConfig {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?,
            jwt_secret: std::env::var("JWT_SECRET")
                .map_err(|_| anyhow::anyhow!("JWT_SECRET must be set"))?,
            encryption_key: std::env::var("ENCRYPTION_KEY")
                .map_err(|_| anyhow::anyhow!("ENCRYPTION_KEY must be set"))?,
            alpaca: AlpacaConfig {
                api_key: std::env::var("ALPACA_API_KEY")
                    .map_err(|_| anyhow::anyhow!("ALPACA_API_KEY must be set"))?,
                api_secret: std::env::var("ALPACA_API_SECRET")
                    .map_err(|_| anyhow::anyhow!("ALPACA_API_SECRET must be set"))?,
            },
            finnhub: FinnhubConfig {
                api_key: std::env::var("FINNHUB_API_KEY")
                    .map_err(|_| anyhow::anyhow!("FINNHUB_API_KEY must be set"))?,
            },
            server: ServerConfig {
                host: std::env::var("SERVER_HOST").unwrap_or_else(|_| default_host()),
                port: std::env::var("SERVER_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_port),
                max_db_connections: std::env::var("MAX_DB_CONNECTIONS")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or_else(default_max_db_connections),
            },
        };

        Ok(config)
    }

    /// Parse the encryption key string into a 32-byte array.
    /// The key should be provided as a 64-character hex string.
    /// Falls back to UTF-8 byte padding for backward compatibility.
    pub fn parse_encryption_key(&self) -> anyhow::Result<[u8; 32]> {
        // Try hex decoding first (recommended: 64-char hex string → 32 bytes)
        if self.encryption_key.len() == 64 {
            if let Ok(bytes) = hex::decode(&self.encryption_key) {
                if let Ok(key) = bytes.try_into() {
                    return Ok(key);
                }
            }
        }

        // Fallback: pad/truncate UTF-8 bytes for backward compatibility
        tracing::warn!(
            "ENCRYPTION_KEY is not a 64-char hex string. Using UTF-8 byte padding (less secure). \
             Consider migrating to a hex-encoded 32-byte key."
        );
        let mut key = [0u8; 32];
        let key_bytes = self.encryption_key.as_bytes();
        let len = 32.min(key_bytes.len());
        key[..len].copy_from_slice(&key_bytes[..len]);
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_server_config() {
        let config = default_server_config();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);
        assert_eq!(config.max_db_connections, 5);
    }

    #[test]
    fn test_parse_encryption_key_hex() {
        let config = AppConfig {
            database_url: String::new(),
            jwt_secret: String::new(),
            encryption_key: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            alpaca: AlpacaConfig { api_key: String::new(), api_secret: String::new() },
            finnhub: FinnhubConfig { api_key: String::new() },
            server: default_server_config(),
        };
        let key = config.parse_encryption_key().unwrap();
        assert_eq!(key, [0u8; 32]);
    }

    #[test]
    fn test_parse_encryption_key_fallback() {
        let config = AppConfig {
            database_url: String::new(),
            jwt_secret: String::new(),
            encryption_key: "short_key".to_string(),
            alpaca: AlpacaConfig { api_key: String::new(), api_secret: String::new() },
            finnhub: FinnhubConfig { api_key: String::new() },
            server: default_server_config(),
        };
        let key = config.parse_encryption_key().unwrap();
        assert_eq!(&key[..9], b"short_key");
        assert_eq!(&key[9..], &[0u8; 23]);
    }
}
