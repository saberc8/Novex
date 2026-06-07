pub mod client_service;
pub mod dept_service;
pub mod dict_service;
pub mod file_service;
pub mod menu_service;
pub mod option_service;
pub mod role_service;
pub mod secret_service;
pub mod storage_service;
pub mod user_service;

use chrono::NaiveDateTime;

use crate::shared::error::AppError;

pub(crate) fn format_datetime(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub(crate) fn format_optional_datetime(value: Option<NaiveDateTime>) -> String {
    value.map(format_datetime).unwrap_or_default()
}

pub(crate) fn trim_to_none(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn ensure_max_chars(field_name: &str, value: &str, max: usize) -> Result<(), AppError> {
    if value.chars().count() > max {
        return Err(AppError::bad_request(format!(
            "{field_name}长度不能超过 {max} 个字符"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod secret_contract_tests {
    use serde_json::json;

    use super::secret_service::{
        mask_secret_value, normalize_secret_command, SecretCommand, SecretRecordPublicResp,
    };

    #[test]
    fn secret_command_masks_plaintext_and_never_exposes_ciphertext() {
        let command = normalize_secret_command(SecretCommand {
            scope_type: " Tenant ".to_owned(),
            scope_id: " 1 ".to_owned(),
            code: " github.connector ".to_owned(),
            plaintext: "github_pat_1234567890".to_owned(),
            metadata: json!({"purpose":"github"}),
            status: 1,
        })
        .expect("secret command should normalize");

        assert_eq!(command.scope_type, "tenant");
        assert_eq!(command.scope_id, "1");
        assert_eq!(command.code, "github.connector");
        assert_eq!(mask_secret_value(&command.plaintext), "gith****7890");

        let response = serde_json::to_value(SecretRecordPublicResp {
            id: 1,
            scope_type: command.scope_type,
            scope_id: command.scope_id,
            code: command.code,
            key_version: 2,
            masked_value: "gith****7890".to_owned(),
            expires_at: None,
            rotated_at: Some("2026-06-06 10:00:00".to_owned()),
            last_used_at: None,
            metadata: command.metadata,
            status: 1,
            create_time: "2026-06-06 09:00:00".to_owned(),
            update_time: Some("2026-06-06 10:00:00".to_owned()),
        })
        .unwrap();

        assert_eq!(response["maskedValue"], "gith****7890");
        assert!(response.get("ciphertext").is_none());
        assert!(response.get("plaintext").is_none());
    }

    #[test]
    fn secret_command_rejects_short_or_blank_plaintext() {
        let err = normalize_secret_command(SecretCommand {
            scope_type: "tenant".to_owned(),
            scope_id: "1".to_owned(),
            code: "github.connector".to_owned(),
            plaintext: "   ".to_owned(),
            metadata: json!({}),
            status: 1,
        })
        .unwrap_err();

        assert!(err.to_string().contains("密钥明文不能为空"));
    }

    #[test]
    fn secret_command_defaults_metadata_to_empty_object() {
        let command = normalize_secret_command(SecretCommand {
            scope_type: "tenant".to_owned(),
            scope_id: "1".to_owned(),
            code: "github.connector".to_owned(),
            plaintext: "github_pat_1234567890".to_owned(),
            metadata: serde_json::Value::Null,
            status: 1,
        })
        .expect("secret command should normalize");

        assert_eq!(command.metadata, json!({}));
    }
}
