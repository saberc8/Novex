pub const WEBHOOK_SIGNATURE_PREFIX: &str = "sha256=";
const MAX_IDEMPOTENCY_KEY_CHARS: usize = 128;

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerValidationError {
    MissingIdempotencyKey,
    IdempotencyKeyTooLong,
}

impl std::fmt::Display for TriggerValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingIdempotencyKey => formatter.write_str("idempotency key is required"),
            Self::IdempotencyKeyTooLong => formatter.write_str("idempotency key is too long"),
        }
    }
}

impl std::error::Error for TriggerValidationError {}

pub fn webhook_signature(secret: &str, body: &[u8]) -> String {
    let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any size");
    hmac::Mac::update(&mut mac, body);
    format!(
        "{WEBHOOK_SIGNATURE_PREFIX}{}",
        hex_encode(&hmac::Mac::finalize(mac).into_bytes())
    )
}

pub fn verify_webhook_signature(secret: &str, body: &[u8], provided: &str) -> bool {
    let provided = provided.trim();
    let digest = provided
        .strip_prefix(WEBHOOK_SIGNATURE_PREFIX)
        .unwrap_or(provided);
    let Some(bytes) = hex_decode(digest) else {
        return false;
    };
    let Ok(mut mac) = <HmacSha256 as hmac::Mac>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    hmac::Mac::update(&mut mac, body);
    hmac::Mac::verify_slice(mac, &bytes).is_ok()
}

pub fn normalize_idempotency_key(raw: &str) -> Result<String, TriggerValidationError> {
    let key = raw.trim();
    if key.is_empty() {
        return Err(TriggerValidationError::MissingIdempotencyKey);
    }
    if key.chars().count() > MAX_IDEMPOTENCY_KEY_CHARS {
        return Err(TriggerValidationError::IdempotencyKeyTooLong);
    }
    Ok(key.to_owned())
}

fn hex_encode(bytes: &[u8]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(CHARS[(byte >> 4) as usize] as char);
        encoded.push(CHARS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
