use crate::ConfigInterceptor;
use cowen_common::{CowenResult, CowenError};

pub struct PortInterceptor;
impl ConfigInterceptor for PortInterceptor {
    fn validate(&self, key: &str, value: &str) -> CowenResult<()> {
        if key.ends_with(".port") || key.contains("port") {
            let port = value.parse::<u16>().map_err(|_| CowenError::Config(format!("Invalid port: {}", value)))?;
            if port < 1024 && port != 0 {
                return Err(CowenError::Config(format!("Port {} out of range (1024-65535)", port)));
            }
        }
        Ok(())
    }
}

pub struct UrlInterceptor;
impl ConfigInterceptor for UrlInterceptor {
    fn validate(&self, key: &str, value: &str) -> CowenResult<()> {
        if (key.ends_with(".target") || key.ends_with(".url"))
            && !value.starts_with("http://")
            && !value.starts_with("https://")
        {
            return Err(CowenError::Config(format!("Invalid URL format: {}", value)));
        }
        Ok(())
    }
}
