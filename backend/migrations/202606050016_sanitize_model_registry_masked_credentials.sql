UPDATE ai_model_credential
SET masked_value = 'configured',
    update_time = NOW()
WHERE masked_value LIKE 'env:%'
   OR masked_value LIKE '%\_API\_KEY%' ESCAPE '\'
   OR masked_value ILIKE '%secret%';
