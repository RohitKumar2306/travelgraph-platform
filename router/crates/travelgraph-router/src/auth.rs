use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub sub: String,
    pub loyalty_tier: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    #[serde(default)]
    loyalty_tier: String,
    #[serde(default)]
    email: String,
}

impl Identity {
    pub fn anonymous() -> Self {
        Self {
            sub: "anonymous".to_string(),
            loyalty_tier: "anonymous".to_string(),
            email: String::new(),
        }
    }

    pub fn from_headers(headers: &HeaderMap, jwt_secret: &str) -> Self {
        let Some(token) = bearer_token(headers) else {
            return Self::anonymous();
        };

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false;
        validation.required_spec_claims.clear();

        match decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &validation,
        ) {
            Ok(data) if !data.claims.sub.trim().is_empty() => Self {
                sub: data.claims.sub,
                loyalty_tier: data.claims.loyalty_tier,
                email: data.claims.email,
            },
            _ => Self::anonymous(),
        }
    }

    pub fn signed_headers(&self, secret: &str) -> Vec<(&'static str, String)> {
        vec![
            ("x-user-id", self.sub.clone()),
            ("x-user-tier", self.loyalty_tier.clone()),
            ("x-user-email", self.email.clone()),
            ("x-identity-signature", self.signature(secret)),
        ]
    }

    fn signature(&self, secret: &str) -> String {
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
        mac.update(canonical_identity(&self.sub, &self.loyalty_tier, &self.email).as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get("authorization")?.to_str().ok()?.trim();
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn canonical_identity(sub: &str, tier: &str, email: &str) -> String {
    format!("{sub}\n{tier}\n{email}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestClaims<'a> {
        sub: &'a str,
        loyalty_tier: &'a str,
        email: &'a str,
    }

    #[test]
    fn valid_jwt_extracts_identity() {
        let secret = "jwt-secret";
        let token = encode(
            &Header::new(Algorithm::HS256),
            &TestClaims {
                sub: "user-1",
                loyalty_tier: "gold",
                email: "a@example.com",
            },
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );

        assert_eq!(
            Identity::from_headers(&headers, secret),
            Identity {
                sub: "user-1".into(),
                loyalty_tier: "gold".into(),
                email: "a@example.com".into(),
            }
        );
    }

    #[test]
    fn invalid_or_missing_jwt_is_anonymous() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            Identity::from_headers(&headers, "secret"),
            Identity::anonymous()
        );
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer not-a-jwt"),
        );
        assert_eq!(
            Identity::from_headers(&headers, "secret"),
            Identity::anonymous()
        );
    }
}
