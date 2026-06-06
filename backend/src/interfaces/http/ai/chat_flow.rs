#[cfg(test)]
mod tests {
    #[test]
    fn chat_flow_migrations_define_session_message_and_permissions() {
        let schema = include_str!(
            "../../../../migrations/202606060002_create_ai_chat_flow.sql"
        );
        let permissions = include_str!(
            "../../../../migrations/202606060003_seed_ai_chat_flow_permissions.sql"
        );

        for table in ["ai_chat_flow_session", "ai_chat_flow_message"] {
            assert!(schema.contains(table), "{table} missing from migration");
        }
        for field in [
            "dataset_id",
            "mode",
            "rag_trace_id",
            "citations",
            "message_count",
        ] {
            assert!(schema.contains(field), "{field} missing from migration");
        }
        for permission in [
            "ai:chatFlow:list",
            "ai:chatFlow:create",
            "ai:chatFlow:message",
        ] {
            assert!(
                permissions.contains(permission),
                "{permission} missing from seed"
            );
        }
    }
}
