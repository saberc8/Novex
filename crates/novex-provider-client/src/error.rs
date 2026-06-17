use std::{error::Error, fmt};

#[derive(Debug)]
pub enum ModelProviderClientError {
    Transport(reqwest::Error),
    HttpStatus {
        failure_message: String,
        status: u16,
    },
    BadResponse(String),
}

impl fmt::Display for ModelProviderClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(err) => write!(f, "{err}"),
            Self::HttpStatus {
                failure_message,
                status,
            } => write!(f, "{failure_message}: HTTP {status}"),
            Self::BadResponse(message) => write!(f, "{message}"),
        }
    }
}

impl Error for ModelProviderClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(err) => Some(err),
            Self::HttpStatus { .. } | Self::BadResponse(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_error_preserves_backend_message_shape() {
        let error = ModelProviderClientError::HttpStatus {
            failure_message: "LLM 模型调用失败".to_owned(),
            status: 429,
        };

        assert_eq!(error.to_string(), "LLM 模型调用失败: HTTP 429");
    }

    #[test]
    fn bad_response_error_preserves_provider_message() {
        let error = ModelProviderClientError::BadResponse("Embedding 模型响应为空".to_owned());

        assert_eq!(error.to_string(), "Embedding 模型响应为空");
    }
}
