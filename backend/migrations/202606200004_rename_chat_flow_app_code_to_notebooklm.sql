-- Rename the app-facing knowledge chat flow namespace after the frontend moved
-- from apps/chat-web to apps/notebooklm.

ALTER TABLE ai_chat_flow_session
    ALTER COLUMN app_code SET DEFAULT 'notebooklm';

UPDATE ai_chat_flow_session
SET app_code = 'notebooklm'
WHERE app_code = 'chat-web';
