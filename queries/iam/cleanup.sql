WITH states AS (DELETE FROM iam_oidc_states WHERE expires_at<NOW()) DELETE FROM iam_sessions WHERE expires_at<NOW();
