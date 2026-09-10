DELETE FROM tracing_spans WHERE received_at < NOW()-make_interval(days=>$1);
