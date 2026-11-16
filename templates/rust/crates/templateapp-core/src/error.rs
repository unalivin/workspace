use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Validation { field: String, problem: String, fix: String },
    NotFound(String),
    Conflict(String),
    Usage(String),
    Storage(String),
    Io(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn invalid(field: &str, problem: impl Into<String>, fix: impl Into<String>) -> Self {
        AppError::Validation { field: field.to_string(), problem: problem.into(), fix: fix.into() }
    }

    pub fn is_usage(&self) -> bool {
        matches!(self, AppError::Validation { .. } | AppError::Usage(_))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Validation { field, problem, fix } => write!(formatter, "{field}: {problem}. {fix}"),
            AppError::NotFound(what) => write!(formatter, "{what} was not found"),
            AppError::Conflict(message) | AppError::Usage(message) | AppError::Storage(message) | AppError::Io(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        match error {
            rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::DatabaseBusy => AppError::Storage(
                "The database is busy: another program is writing to it. Wait a moment and try again.".to_string(),
            ),
            other => AppError::Storage(format!("database error: {other}")),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Io(error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::Io(format!("invalid JSON: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_errors_name_the_field_the_problem_and_the_fix() {
        let error = AppError::invalid("name", "the name is empty", "Enter a name of 1 to 200 characters");
        assert_eq!(error.to_string(), "name: the name is empty. Enter a name of 1 to 200 characters");
        assert!(error.is_usage());
        assert!(!AppError::Storage("x".to_string()).is_usage());
        assert_eq!(AppError::NotFound("record 7".to_string()).to_string(), "record 7 was not found");
    }
}
