//! Identity values are opaque to IAM. Provider adapters construct and interpret them.
use crate::proto::tilde::types::v1::IdentityType;
use connectrpc::ConnectError;

pub struct Identity {
    pub identity_type: IdentityType,
    pub value: String,
}
impl Identity {
    /// Only storage bounds are shared; syntax and canonical values belong to the provider.
    pub fn validate(&self) -> Result<(), ConnectError> {
        if self.identity_type == IdentityType::Unspecified
            || self.value.is_empty()
            || self.value.len() > 320
        {
            return Err(ConnectError::invalid_argument("Invalid identity value"));
        }
        Ok(())
    }
}
pub fn kind_name(kind: IdentityType) -> &'static str {
    match kind {
        IdentityType::Email => "email",
        IdentityType::PhoneNumber => "phone_number",
        IdentityType::Username => "username",
        _ => "unspecified",
    }
}
pub fn kind_value(kind: &str) -> IdentityType {
    match kind {
        "email" => IdentityType::Email,
        "phone_number" => IdentityType::PhoneNumber,
        "username" => IdentityType::Username,
        _ => IdentityType::Unspecified,
    }
}
pub struct VerificationMessage<'a> {
    pub value: &'a str,
    pub text: &'a str,
    pub url: &'a str,
    pub template_name: Option<&'a str>,
    pub template_language: Option<&'a str>,
}
