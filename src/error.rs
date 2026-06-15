use std::fmt;

#[derive(Debug)]
pub enum PluginError {
    Config(String),
    Http(String),
    Api(String),
    Parse(String),
    Client(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::Config(m) => write!(f, "Configuration error: {}", m),
            PluginError::Http(m) => write!(f, "HTTP error: {}", m),
            PluginError::Api(m) => write!(f, "Cloudflare D1 API error: {}", m),
            PluginError::Parse(m) => write!(f, "Parse error: {}", m),
            PluginError::Client(m) => write!(f, "Client error: {}", m),
        }
    }
}

impl From<PluginError> for String {
    fn from(e: PluginError) -> Self {
        e.to_string()
    }
}
