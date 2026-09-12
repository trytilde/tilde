DELETE FROM sidecar_directives WHERE acked_at<NOW()-INTERVAL '1 hour' OR created_at<NOW()-INTERVAL '7 days';
