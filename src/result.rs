use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Success,
    Failure,
    MissingRecord,
    DuplicateRecord,
    InvalidState,
}

impl ResultStatus {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::MissingRecord => 3,
            Self::DuplicateRecord => 4,
            Self::InvalidState => 5,
        }
    }

    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultError {
    pub code: ResultStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandResult<T> {
    pub status: ResultStatus,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResultError>,
}

impl<T> CommandResult<T> {
    pub fn success(data: Option<T>, message: Option<String>) -> Self {
        Self {
            status: ResultStatus::Success,
            ok: true,
            message,
            data,
            error: None,
        }
    }

    pub fn error(status: ResultStatus, message: impl Into<String>) -> Self {
        assert!(!status.is_success());
        let message = message.into();
        Self {
            status,
            ok: false,
            message: None,
            data: None,
            error: Some(ResultError {
                code: status,
                message,
            }),
        }
    }

    pub fn human_message(&self) -> &str {
        self.error
            .as_ref()
            .map(|error| error.message.as_str())
            .or(self.message.as_deref())
            .unwrap_or("ok")
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("could not render JSON result: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn render_json<T: Serialize>(result: &CommandResult<T>) -> Result<String, RenderError> {
    Ok(serde_json::to_string(result)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_success_result_schema() {
        let result = CommandResult::success(Some(json!({"name": "dev", "state": "running"})), None);
        assert_eq!(
            render_json(&result).expect("result should serialize"),
            r#"{"status":"success","ok":true,"data":{"name":"dev","state":"running"}}"#
        );
    }

    #[test]
    fn serializes_error_result_schema() {
        let result = CommandResult::<()>::error(ResultStatus::MissingRecord, "no such process");
        assert_eq!(
            render_json(&result).expect("result should serialize"),
            r#"{"status":"missing_record","ok":false,"error":{"code":"missing_record","message":"no such process"}}"#
        );
    }
}
