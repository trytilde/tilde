use envconfig::Envconfig;
use std::collections::HashMap;
use tilde::config::Config;

#[test]
fn typed_environment_validates_before_initializing_encryption() {
    let base = HashMap::from([
        ("ENGINE_OIDC_ISSUER".into(), "https://issuer.example".into()),
        ("ENGINE_OIDC_CLIENT_ID".into(), "client".into()),
        ("ENGINE_OIDC_CLIENT_SECRET".into(), "test-secret".into()),
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
    for (name, value) in [
        ("WEB_PORT", "65536"),
        ("INGRESS_PORT", "65536"),
        ("API_PORT", "not-a-port"),
        ("ENGINE_ALLOW_NETWORK", "perhaps"),
        ("ENGINE_MANAGEMENT_LISTEN", "invalid"),
        ("ENGINE_EVENT_INGRESS_LISTEN", "invalid"),
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
        ("INGRESS_PORT", "0"),
        ("INGRESS_PORT", "8080"),
        ("INGRESS_PORT", "5173"),
        ("WEB_PORT", "8080"),
        ("ENGINE_ENCRYPTION_KEY", "private-invalid-key"),
        ("DATABASE_URL", ""),
        ("ENGINE_EVENT_INGRESS_LISTEN", "127.0.0.1:8080"),
        ("ENGINE_EVENT_INGRESS_LISTEN", "127.0.0.1:8081"),
        (
            "ENGINE_EVENT_INGRESS_PUBLIC_URL",
            "https://events.example/path",
        ),
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

#[test]
fn runtime_only_configuration_does_not_require_oidc_or_management_ports() {
    let mut values = HashMap::from([
        (
            "DATABASE_URL".into(),
            "postgres://engine@localhost/test".into(),
        ),
        (
            "ENGINE_ENCRYPTION_KEY".into(),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
        ),
        ("ENGINE_MANAGEMENT_ENABLED".into(), "false".into()),
        ("ENGINE_WEB_ENABLED".into(), "false".into()),
        ("API_PORT".into(), "0".into()),
        ("WEB_PORT".into(), "0".into()),
    ]);
    assert!(
        Config::init_from_hashmap(&values)
            .unwrap()
            .key_protection()
            .is_ok()
    );
    values.insert("ENGINE_MANAGEMENT_ENABLED".into(), "true".into());
    assert!(
        Config::init_from_hashmap(&values)
            .unwrap()
            .key_protection()
            .err()
            .unwrap()
            .to_string()
            .contains("ENGINE_OIDC_ISSUER")
    );
}
