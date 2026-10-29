-- Pre-release runtime settings intentionally reset to the independent file/execute model.
-- Subsequent saves use the new strict shape; no legacy JSON fallback is retained.
DELETE FROM runtime_access_settings;
