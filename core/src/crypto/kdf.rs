//! Password-based key derivation with Argon2id (SPEC §6.1).

use std::time::{Duration, Instant};

use argon2::{Algorithm, Argon2, Params, Version};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN, Kek, random_bytes};

/// Salt length in bytes.
pub const SALT_LEN: usize = 16;

/// Length of [`KdfParams::to_bytes`] output.
pub const PARAMS_LEN: usize = 13;

/// Target duration of one derivation when calibrating a new vault (SPEC: ~0.5–1 s).
pub const CALIBRATION_TARGET: Duration = Duration::from_millis(750);

/// Format tag of serialized params: Argon2id, version 0x13.
const ARGON2ID_V13: u8 = 1;

/// Argon2id cost parameters, stored in the vault header so they can be raised later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_kib: u32,
    /// Number of passes.
    pub t: u32,
    /// Degree of parallelism.
    pub p: u32,
}

impl KdfParams {
    /// SPEC default: m = 64 MiB, t = 3, p = 1. Floor for newly created vaults.
    pub const DEFAULT: Self = Self {
        m_kib: 64 * 1024,
        t: 3,
        p: 1,
    };

    /// Upper bounds accepted when reading a header. They cap the work an
    /// attacker-controlled header can force on the device (memory exhaustion).
    pub const MAX_M_KIB: u32 = 1024 * 1024; // 1 GiB
    pub const MAX_T: u32 = 64;
    pub const MAX_P: u32 = 8;

    /// Highest `t` that calibration will pick.
    const MAX_CALIBRATED_T: u32 = 16;

    /// Validated constructor. Allows values below [`Self::DEFAULT`] (tests,
    /// old vaults); vault creation uses [`calibrate`], which never goes below it.
    pub fn new(m_kib: u32, t: u32, p: u32) -> Result<Self, CryptoError> {
        let params = Self { m_kib, t, p };
        params.validate()?;
        Ok(params)
    }

    fn validate(&self) -> Result<(), CryptoError> {
        let ok = (1..=Self::MAX_P).contains(&self.p)
            && (1..=Self::MAX_T).contains(&self.t)
            && self.m_kib <= Self::MAX_M_KIB
            && self.m_kib >= 8 * self.p;
        if ok {
            Ok(())
        } else {
            Err(CryptoError::InvalidKdfParams)
        }
    }

    /// Header encoding: `alg (1 B) || m_kib (u32 LE) || t (u32 LE) || p (u32 LE)`.
    pub fn to_bytes(&self) -> [u8; PARAMS_LEN] {
        let mut out = [0u8; PARAMS_LEN];
        out[0] = ARGON2ID_V13;
        out[1..5].copy_from_slice(&self.m_kib.to_le_bytes());
        out[5..9].copy_from_slice(&self.t.to_le_bytes());
        out[9..13].copy_from_slice(&self.p.to_le_bytes());
        out
    }

    /// Parses and validates the header encoding.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let bytes: &[u8; PARAMS_LEN] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKdfParams)?;
        if bytes[0] != ARGON2ID_V13 {
            return Err(CryptoError::InvalidKdfParams);
        }
        let u32_at =
            |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        Self::new(u32_at(1), u32_at(5), u32_at(9))
    }

    /// Scales `t` so that one derivation takes about `target`, given that
    /// these params took `measured`. Never lowers `t`.
    fn scaled_to(self, measured: Duration, target: Duration) -> Self {
        if measured >= target || measured.is_zero() {
            return self;
        }
        let factor = target.as_secs_f64() / measured.as_secs_f64();
        let t = (f64::from(self.t) * factor).round() as u32;
        Self {
            t: t.clamp(self.t, Self::MAX_CALIBRATED_T.max(self.t)),
            ..self
        }
    }
}

/// Generates a random salt.
pub fn generate_salt() -> Result<[u8; SALT_LEN], CryptoError> {
    let mut salt = [0u8; SALT_LEN];
    random_bytes(&mut salt)?;
    Ok(salt)
}

/// Derives `KEK_pwd` from a password.
///
/// The password is NFC-normalized first, so the same password typed on
/// different keyboards (precomposed vs. combining diacritics) yields the same key.
pub fn derive_kek(
    password: &str,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
) -> Result<Kek, CryptoError> {
    params.validate()?;
    let normalized: Zeroizing<String> = Zeroizing::new(password.nfc().collect());
    let argon_params = Params::new(params.m_kib, params.t, params.p, Some(KEY_LEN))
        .map_err(|_| CryptoError::InvalidKdfParams)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = [0u8; KEY_LEN];
    let result = argon.hash_password_into(normalized.as_bytes(), salt, &mut out);
    let kek = Kek::from_bytes(&mut out);
    result.map_err(|_| CryptoError::Internal)?;
    Ok(kek)
}

/// Picks params for a new vault: [`KdfParams::DEFAULT`] with `t` raised until
/// a derivation takes roughly `target` on this device (SPEC: ~0.5–1 s).
pub fn calibrate(target: Duration) -> Result<KdfParams, CryptoError> {
    let params = KdfParams::DEFAULT;
    let salt = [0u8; SALT_LEN];
    let start = Instant::now();
    derive_kek("calibration", &salt, &params)?;
    Ok(params.scaled_to(start.elapsed(), target))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap params so the test suite stays fast. Never used outside tests.
    const FAST: KdfParams = KdfParams {
        m_kib: 64,
        t: 1,
        p: 1,
    };
    const SALT: [u8; SALT_LEN] = [0x5a; SALT_LEN];

    #[test]
    fn same_input_same_key() {
        let a = derive_kek("correct horse", &SALT, &FAST).unwrap();
        let b = derive_kek("correct horse", &SALT, &FAST).unwrap();
        assert_eq!(a.expose(), b.expose());
    }

    #[test]
    fn different_password_salt_or_params_differ() {
        let base = derive_kek("correct horse", &SALT, &FAST).unwrap();
        let pwd = derive_kek("correct horsf", &SALT, &FAST).unwrap();
        let salt = derive_kek("correct horse", &[0x5b; SALT_LEN], &FAST).unwrap();
        let params = derive_kek("correct horse", &SALT, &KdfParams { t: 2, ..FAST }).unwrap();
        assert_ne!(base.expose(), pwd.expose());
        assert_ne!(base.expose(), salt.expose());
        assert_ne!(base.expose(), params.expose());
    }

    #[test]
    fn password_is_nfc_normalized() {
        let precomposed = derive_kek("za\u{17c}\u{f3}\u{142}\u{107}", &SALT, &FAST).unwrap(); // "zażółć"
        let combining = derive_kek("zaz\u{307}o\u{301}\u{142}c\u{301}", &SALT, &FAST).unwrap();
        assert_eq!(precomposed.expose(), combining.expose());
    }

    #[test]
    fn params_round_trip() {
        for p in [
            KdfParams::DEFAULT,
            FAST,
            KdfParams {
                m_kib: 1 << 20,
                t: 64,
                p: 8,
            },
        ] {
            assert_eq!(KdfParams::from_bytes(&p.to_bytes()).unwrap(), p);
        }
    }

    #[test]
    fn params_encoding_is_stable() {
        assert_eq!(
            hex::encode(KdfParams::DEFAULT.to_bytes()),
            "01000001000300000001000000"
        );
    }

    #[test]
    fn hostile_params_are_rejected() {
        let mut bytes = KdfParams::DEFAULT.to_bytes();
        bytes[0] = 2;
        assert_eq!(
            KdfParams::from_bytes(&bytes),
            Err(CryptoError::InvalidKdfParams)
        );
        assert!(KdfParams::from_bytes(&bytes[..12]).is_err());
        assert!(KdfParams::new(KdfParams::MAX_M_KIB + 1, 3, 1).is_err());
        assert!(KdfParams::new(65536, 0, 1).is_err());
        assert!(KdfParams::new(65536, 65, 1).is_err());
        assert!(KdfParams::new(65536, 3, 0).is_err());
        assert!(KdfParams::new(65536, 3, 9).is_err());
        assert!(KdfParams::new(15, 1, 2).is_err());
    }

    #[test]
    fn scaling_only_raises_t() {
        let d = KdfParams::DEFAULT;
        let ms = Duration::from_millis;
        assert_eq!(d.scaled_to(ms(1000), ms(750)), d);
        assert_eq!(d.scaled_to(ms(250), ms(750)).t, 9);
        assert_eq!(d.scaled_to(ms(1), ms(750)).t, KdfParams::MAX_CALIBRATED_T);
        assert_eq!(d.scaled_to(Duration::ZERO, ms(750)), d);
        assert_eq!(d.scaled_to(ms(250), ms(750)).m_kib, d.m_kib);
    }

    #[test]
    fn calibrate_never_goes_below_default() {
        let p = calibrate(Duration::from_millis(1)).unwrap();
        assert_eq!(p, KdfParams::DEFAULT);
    }

    /// Fixed vector for the derivation (Argon2id over NFC bytes).
    #[test]
    fn fixed_vector() {
        let kek = derive_kek("mossbook", &SALT, &FAST).unwrap();
        assert_eq!(hex::encode(kek.expose()), FIXED_VECTOR_HEX);
    }

    /// Cross-checked with libsodium / argon2-cffi / stdlib HKDF: `tools/verify-vectors.py`.
    const FIXED_VECTOR_HEX: &str =
        "42f38f1d97f523ad2170abeddfcd7c24fd8370c9b3c6408653337c0fd93896f6";
}
