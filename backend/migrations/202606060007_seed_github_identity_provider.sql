-- GitHub login identity provider POC. Repository access is still handled by
-- ai_connector / ai_connector_credential and must not reuse login credentials.

INSERT INTO sys_identity_provider
    (id, tenant_id, provider_type, code, name, client_id, secret_ref, allowed_domains, tenant_policy, status, create_user, create_time)
VALUES
    (1097001, 1, 'github', 'github.login', 'GitHub Login',
     NULLIF(current_setting('novex.github_oauth_client_id', TRUE), ''),
     'env:GITHUB_OAUTH_CLIENT_SECRET',
     '[]'::jsonb,
     '{
        "poc": true,
        "authorizationUrl": "https://github.com/login/oauth/authorize",
        "tokenUrl": "https://github.com/login/oauth/access_token",
        "userInfoUrl": "https://api.github.com/user",
        "defaultScopes": ["read:user", "user:email"],
        "credentialBoundary": "login_identity_only_not_repo_connector"
      }'::jsonb,
     1, 1, NOW())
ON CONFLICT DO NOTHING;
