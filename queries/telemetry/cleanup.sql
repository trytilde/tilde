DELETE FROM telemetry_delivery WHERE created_at<NOW()-INTERVAL '7 days';
