use envconfig::Envconfig;
use std::collections::HashMap;
use tilde::config::Config;

/// ClickHouse and the telemetry buckets every engine requires.
const TELEMETRY: [(&str, &str); 4] = [
    ("ENGINE_CLICKHOUSE_URL", "http://127.0.0.1:8123"),
    ("ENGINE_LOGS_S3_BUCKET", "logs"),
    ("ENGINE_TRACES_S3_BUCKET", "traces"),
    ("ENGINE_METRICS_S3_BUCKET", "metrics"),
];

#[test]
fn typed_environment_validates_before_initializing_encryption() {
    let mut base: HashMap<String, String> =
        TELEMETRY.map(|(k, v)| (k.to_owned(), v.to_owned())).into();
    base.extend([
        (
            "DATABASE_URL".into(),
            "postgres://fixture:password@localhost/fixture".into(),
        ),
        (
            "ENGINE_ENCRYPTION_KEY".into(),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
        ),
    ]);
    assert!(
        Config::init_from_hashmap(&base)
            .unwrap()
            .key_protection()
            .is_ok()
    );
    // There is no telemetry opt-out: each piece refuses startup by name.
    for (name, _) in TELEMETRY {
        let mut values = base.clone();
        values.remove(name);
        let error = Config::init_from_hashmap(&values)
            .unwrap()
            .key_protection()
            .err()
            .expect("telemetry is required");
        assert!(error.to_string().contains(name));
    }
    for (name, value) in [
        ("WEB_PORT", "65536"),
        ("API_PORT", "not-a-port"),
        ("ENGINE_ALLOW_NETWORK", "perhaps"),
        ("ENGINE_LISTEN", "invalid"),
        ("ENGINE_SERVE", "management,gossip"),
        ("ENGINE_ENCRYPTION_BACKEND", "invalid"),
    ] {
        let mut values = base.clone();
        values.insert(name.into(), value.into());
        let error = Config::init_from_hashmap(&values)
            .err()
            .expect("invalid environment must fail");
        assert!(error.to_string().contains(name));
    }
    for (name, value) in [
        ("WEB_PORT", "0"),
        ("WEB_PORT", "8080"),
        ("ENGINE_ENCRYPTION_KEY", "private-invalid-key"),
        ("DATABASE_URL", ""),
        ("ENGINE_INGRESS_PUBLIC_URL", "https://events.example/path"),
        ("ENGINE_PUBLIC_URL", "ftp://tilde.example"),
    ] {
        let mut values = base.clone();
        values.insert(name.into(), value.into());
        let error = Config::init_from_hashmap(&values)
            .unwrap()
            .key_protection()
            .err()
            .expect("invalid configuration must fail");
        assert!(!error.to_string().contains("private-invalid-key"));
    }
    let mut kms = base;
    kms.remove("ENGINE_ENCRYPTION_KEY");
    kms.insert("ENGINE_ENCRYPTION_BACKEND".into(), "aws-kms".into());
    kms.insert("ENGINE_KMS_KEY_ID".into(), "alias/fixture".into());
    assert!(
        Config::init_from_hashmap(&kms)
            .unwrap()
            .key_protection()
            .is_err()
    );
    kms.insert("AWS_DEFAULT_REGION".into(), "us-east-1".into());
    assert!(
        Config::init_from_hashmap(&kms)
            .unwrap()
            .key_protection()
            .is_ok()
    );
}
