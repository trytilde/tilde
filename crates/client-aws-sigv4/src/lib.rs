//! Focused AWS Signature Version 4 signing without the AWS SDK or Smithy.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::{Method, Request, header};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, time::SystemTime};
use url::Url;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials([REDACTED])")
    }
}

#[derive(Clone, Debug)]
pub struct Signer {
    credentials: Credentials,
    region: String,
    service: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresignedRequest {
    pub url: Url,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid AWS signing URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("invalid AWS request header: {0}")]
    Header(#[from] reqwest::header::InvalidHeaderValue),
    #[error("AWS signing URL has no host")]
    MissingHost,
    #[error("AWS signing expiry must be between 1 and 604800 seconds")]
    InvalidExpiry,
}

impl Signer {
    pub fn new(
        credentials: Credentials,
        region: impl Into<String>,
        service: impl Into<String>,
    ) -> Self {
        Self {
            credentials,
            region: region.into(),
            service: service.into(),
        }
    }

    pub fn sign_request_at(
        &self,
        request: &mut Request,
        body: &[u8],
        time: SystemTime,
    ) -> Result<(), Error> {
        let timestamp: DateTime<Utc> = time.into();
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
        let date = timestamp.format("%Y%m%d").to_string();
        let payload_hash = sha256_hex(body);
        request
            .headers_mut()
            .insert("x-amz-date", header::HeaderValue::from_str(&amz_date)?);
        request.headers_mut().insert(
            "x-amz-content-sha256",
            header::HeaderValue::from_str(&payload_hash)?,
        );
        if let Some(token) = &self.credentials.session_token {
            request.headers_mut().insert(
                "x-amz-security-token",
                header::HeaderValue::from_str(token)?,
            );
        }
        let (canonical_headers, signed_headers) =
            canonical_headers(request.url(), request.headers())?;
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            request.method(),
            canonical_uri(request.url()),
            canonical_query(request.url()),
            canonical_headers,
            signed_headers,
            payload_hash
        );
        let scope = self.scope(&date);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={}",
            self.credentials.access_key_id,
            self.signature(&date, &string_to_sign)
        );
        request.headers_mut().insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&authorization)?,
        );
        Ok(())
    }

    pub fn presign_at(
        &self,
        method: Method,
        mut url: Url,
        headers: BTreeMap<String, String>,
        expires_seconds: u64,
        time: SystemTime,
    ) -> Result<PresignedRequest, Error> {
        if !(1..=604_800).contains(&expires_seconds) {
            return Err(Error::InvalidExpiry);
        }
        let timestamp: DateTime<Utc> = time.into();
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
        let date = timestamp.format("%Y%m%d").to_string();
        let scope = self.scope(&date);
        let (canonical_headers, signed_headers) = canonical_headers_from_map(&url, &headers)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("X-Amz-Algorithm", "AWS4-HMAC-SHA256");
            query.append_pair(
                "X-Amz-Credential",
                &format!("{}/{scope}", self.credentials.access_key_id),
            );
            query.append_pair("X-Amz-Date", &amz_date);
            query.append_pair("X-Amz-Expires", &expires_seconds.to_string());
            query.append_pair("X-Amz-SignedHeaders", &signed_headers);
            if let Some(token) = &self.credentials.session_token {
                query.append_pair("X-Amz-Security-Token", token);
            }
        }
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\nUNSIGNED-PAYLOAD",
            method,
            canonical_uri(&url),
            canonical_query(&url),
            canonical_headers,
            signed_headers
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );
        let signature = self.signature(&date, &string_to_sign);
        url.query_pairs_mut()
            .append_pair("X-Amz-Signature", &signature);
        Ok(PresignedRequest { url, headers })
    }

    fn scope(&self, date: &str) -> String {
        format!("{date}/{}/{}/aws4_request", self.region, self.service)
    }
    fn signature(&self, date: &str, value: &str) -> String {
        let initial_key = Zeroizing::new(format!("AWS4{}", self.credentials.secret_access_key));
        let date_key = Zeroizing::new(hmac(initial_key.as_bytes(), date.as_bytes()));
        let region_key = Zeroizing::new(hmac(&date_key, self.region.as_bytes()));
        let service_key = Zeroizing::new(hmac(&region_key, self.service.as_bytes()));
        let signing_key = Zeroizing::new(hmac(&service_key, b"aws4_request"));
        hex::encode(hmac(&signing_key, value.as_bytes()))
    }
}

fn canonical_headers(url: &Url, headers: &header::HeaderMap) -> Result<(String, String), Error> {
    canonical_header_values(
        url,
        headers
            .iter()
            .filter(|(n, _)| *n != header::AUTHORIZATION)
            .map(|(n, v)| {
                (
                    n.as_str().to_ascii_lowercase(),
                    normalize(v.to_str().unwrap_or_default()),
                )
            })
            .collect(),
    )
}
fn canonical_headers_from_map(
    url: &Url,
    headers: &BTreeMap<String, String>,
) -> Result<(String, String), Error> {
    canonical_header_values(
        url,
        headers
            .iter()
            .map(|(n, v)| (n.to_ascii_lowercase(), normalize(v)))
            .collect(),
    )
}
fn canonical_header_values(
    url: &Url,
    mut values: BTreeMap<String, String>,
) -> Result<(String, String), Error> {
    values
        .entry("host".into())
        .or_insert_with(|| match url.port() {
            Some(p) => format!("{}:{p}", url.host_str().unwrap_or_default()),
            None => url.host_str().unwrap_or_default().into(),
        });
    if values["host"].is_empty() {
        return Err(Error::MissingHost);
    }
    let signed = values.keys().cloned().collect::<Vec<_>>().join(";");
    let canonical = values.iter().map(|(n, v)| format!("{n}:{v}\n")).collect();
    Ok((canonical, signed))
}
fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn canonical_uri(url: &Url) -> &str {
    if url.path().is_empty() {
        "/"
    } else {
        url.path()
    }
}
fn canonical_query(url: &Url) -> String {
    let mut pairs = url
        .query_pairs()
        .map(|(k, v)| (aws_encode(k.as_bytes()), aws_encode(v.as_bytes())))
        .collect::<Vec<_>>();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}
pub fn aws_encode(value: &[u8]) -> String {
    let mut out = String::new();
    for b in value {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(*b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
fn sha256_hex(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}
fn hmac(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("valid HMAC key");
    mac.update(value);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};
    fn signer() -> Signer {
        Signer::new(
            Credentials {
                access_key_id: "AKIDEXAMPLE".into(),
                secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
                session_token: None,
            },
            "us-east-1",
            "iam",
        )
    }
    #[test]
    fn signs_expected_scope_and_headers() {
        let mut request = reqwest::Client::new()
            .get("https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08")
            .header(
                "content-type",
                "application/x-www-form-urlencoded; charset=utf-8",
            )
            .build()
            .unwrap();
        signer()
            .sign_request_at(
                &mut request,
                &[],
                UNIX_EPOCH + Duration::from_secs(1_448_250_400),
            )
            .unwrap();
        let auth = request.headers()[header::AUTHORIZATION].to_str().unwrap();
        assert!(auth.contains("Credential=AKIDEXAMPLE/20151123/us-east-1/iam/aws4_request"));
        assert!(auth.contains("SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date"));
        // Fixed vector independently computed with Python hashlib/hmac.
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20151123/us-east-1/iam/aws4_request, SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature=dbf58bfffd80cb426e938e92f05fc55d186f79a6e07506afbfc4c1315a999c37"
        );
    }
    #[test]
    fn presigns_session_token() {
        let request = Signer::new(
            Credentials {
                access_key_id: "AKIDEXAMPLE".into(),
                secret_access_key: "secret".into(),
                session_token: Some("token/+=".into()),
            },
            "auto",
            "s3",
        )
        .presign_at(
            Method::PUT,
            Url::parse("https://localhost:9000/bucket/file").unwrap(),
            BTreeMap::from([("content-type".into(), "text/plain".into())]),
            900,
            UNIX_EPOCH + Duration::from_secs(1_448_250_400),
        )
        .unwrap();
        assert!(
            request
                .url
                .as_str()
                .contains("X-Amz-Security-Token=token%2F%2B%3D")
        );
    }
}
