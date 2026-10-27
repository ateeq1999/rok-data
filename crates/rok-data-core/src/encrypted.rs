//! Columns encrypted at rest: [`Encrypted`], sealed with AES-256-GCM under
//! one process-wide key, so a database dump holds ciphertext only.

use std::fmt;
use std::sync::OnceLock;

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
use sqlx::{Decode, Encode, Postgres, Type};

use crate::{Error, Result, Value};

/// The environment variable [`Encrypted`] reads its key from when
/// [`set_encryption_key`] wasn't called: 32 bytes, standard base64
/// (`openssl rand -base64 32`).
pub const ENCRYPTION_KEY_VAR: &str = "ROK_DATA_ENCRYPTION_KEY";

static CIPHER: OnceLock<Aes256Gcm> = OnceLock::new();

/// Set the key every [`Encrypted`] column is sealed with, once, at
/// start-up. Without it, the key is read from [`ENCRYPTION_KEY_VAR`] on
/// first use. A second call is an [`Error::Config`]: rows sealed under one
/// key can't be opened with another.
pub fn set_encryption_key(key: &[u8; 32]) -> Result<()> {
    CIPHER
        .set(Aes256Gcm::new(key.into()))
        .map_err(|_| Error::Config("the encryption key is already set".into()))
}

fn cipher() -> std::result::Result<&'static Aes256Gcm, BoxDynError> {
    if let Some(cipher) = CIPHER.get() {
        return Ok(cipher);
    }
    let encoded = std::env::var(ENCRYPTION_KEY_VAR).map_err(|_| {
        format!("no encryption key: call `set_encryption_key` or set `{ENCRYPTION_KEY_VAR}`")
    })?;
    let key: [u8; 32] = STANDARD
        .decode(encoded.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| format!("`{ENCRYPTION_KEY_VAR}` is not 32 bytes in base64"))?;
    Ok(CIPHER.get_or_init(|| Aes256Gcm::new(&key.into())))
}

/// `base64(nonce ‖ ciphertext)` of `plain`, under a fresh random nonce.
fn seal(plain: &str) -> std::result::Result<String, BoxDynError> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let mut sealed = nonce.to_vec();
    sealed.extend(
        cipher()?
            .encrypt(&nonce, plain.as_bytes())
            .map_err(|_| "encrypting a column failed")?,
    );
    Ok(STANDARD.encode(sealed))
}

fn open(sealed: &str) -> std::result::Result<String, BoxDynError> {
    let unreadable = "an encrypted column can't be decrypted (another key, or not ciphertext)";
    let bytes = STANDARD.decode(sealed).map_err(|_| unreadable)?;
    if bytes.len() <= 12 {
        return Err(unreadable.into());
    }
    let (nonce, ciphertext) = bytes.split_at(12);
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| unreadable)?;
    let plain = cipher()?
        .decrypt(&Nonce::from(nonce), ciphertext)
        .map_err(|_| unreadable)?;
    Ok(String::from_utf8(plain).map_err(|_| unreadable)?)
}

/// A text column stored encrypted: the field holds the plain text, the
/// table holds `base64(nonce ‖ AES-256-GCM ciphertext)` in a `TEXT`
/// column. Writes seal it, reads open it.
///
/// ```ignore
/// #[derive(Model)]
/// struct Account {
///     id: i64,
///     iban: Encrypted,
/// }
/// rok_data::set_encryption_key(&key)?; // or ROK_DATA_ENCRYPTION_KEY
/// let account = Account { id: 0, iban: "DE89…".into() }.insert(&db).await?;
/// assert_eq!(account.iban.as_str(), "DE89…");
/// ```
///
/// Every write uses a fresh nonce, so the column can't be filtered or
/// indexed by value. `Debug` hides the text.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Encrypted(String);

impl Encrypted {
    /// Wrap a plain text.
    pub fn new(plain: impl Into<String>) -> Self {
        Self(plain.into())
    }

    /// The plain text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The plain text, unwrapped.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for Encrypted {
    fn from(plain: String) -> Self {
        Self(plain)
    }
}

impl From<&str> for Encrypted {
    fn from(plain: &str) -> Self {
        Self(plain.to_owned())
    }
}

impl std::ops::Deref for Encrypted {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Encrypted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Encrypted(***)")
    }
}

/// The sealed text, as the column holds it.
#[derive(Clone)]
struct Sealed(Encrypted);

impl fmt::Debug for Sealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Type<Postgres> for Sealed {
    fn type_info() -> PgTypeInfo {
        <String as Type<Postgres>>::type_info()
    }
}

impl Encode<'_, Postgres> for Sealed {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> std::result::Result<IsNull, BoxDynError> {
        Encode::<Postgres>::encode_by_ref(&seal(self.0.as_str())?, buf)
    }
}

impl Type<Postgres> for Encrypted {
    fn type_info() -> PgTypeInfo {
        <String as Type<Postgres>>::type_info()
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        <String as Type<Postgres>>::compatible(ty)
    }
}

impl Decode<'_, Postgres> for Encrypted {
    fn decode(value: PgValueRef<'_>) -> std::result::Result<Self, BoxDynError> {
        let sealed = <String as Decode<Postgres>>::decode(value)?;
        Ok(Self(open(&sealed)?))
    }
}

impl Encode<'_, Postgres> for Encrypted {
    fn encode_by_ref(
        &self,
        buf: &mut PgArgumentBuffer,
    ) -> std::result::Result<IsNull, BoxDynError> {
        Sealed(self.clone()).encode_by_ref(buf)
    }
}

impl From<&Encrypted> for Value {
    fn from(value: &Encrypted) -> Self {
        Value::custom(Some(Sealed(value.clone())))
    }
}

impl From<&Option<Encrypted>> for Value {
    fn from(value: &Option<Encrypted>) -> Self {
        Value::custom(value.clone().map(Sealed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealing_round_trips_and_differs_each_time() {
        let _ = set_encryption_key(&[7; 32]);
        let first = seal("secret").unwrap();
        let second = seal("secret").unwrap();
        assert_ne!(first, second, "a fresh nonce each time");
        assert!(!first.contains("secret"));
        assert_eq!(open(&first).unwrap(), "secret");
        assert!(open("not ciphertext").is_err());
        assert_eq!(format!("{:?}", Encrypted::new("secret")), "Encrypted(***)");
    }
}
