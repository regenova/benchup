//! Challenge-response pairing with the `benchup bridge` (protocol 2).
//!
//! The bridge listens on 127.0.0.1; BenchUp (the app) and command-line
//! clients connect to it. The raw pairing token is never sent: each side
//! proves it knows the token with an HMAC-SHA256 over both nonces, and the
//! proofs are separated by role so one side's proof cannot be replayed as
//! another's.
use sha2::{Digest, Sha256};

pub const PROTOCOL: u32 = 2;

/// The port the bridge listens on and the app connects to by default.
pub const DEFAULT_PORT: u16 = 47615;
/// The longest client name shown to the designer.
pub const MAX_CLIENT_CHARS: usize = 64;
/// The largest message the app accepts from the bridge; the bridge refuses
/// larger calls itself rather than forwarding them and losing the connection.
pub const MAX_APP_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// The largest message the bridge accepts from a paired peer (scene dumps,
/// rendered views). The app answers with an error rather than send a larger
/// result, which would make the bridge drop the connection.
pub const MAX_BRIDGE_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

/// Machine-readable `code`s of the bridge's `error` frames, so the app can
/// show the reason in the designer's language.
pub mod refusal {
    pub const UNKNOWN_ROLE: &str = "unknown_role";
    pub const TOKEN_MISMATCH: &str = "token_mismatch";
    pub const APP_BUSY: &str = "app_busy";
    pub const UNEXPECTED: &str = "unexpected";
}

const BLOCK: usize = 64;

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    outer.finalize().into()
}

/// Lowercase hex encoding.
pub fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 15) as usize] as char);
    }
    s
}

fn proof(role: &str, token: &str, bridge_nonce: &str, app_nonce: &str) -> String {
    let message = format!("benchup-agent-link/{role}\n{bridge_nonce}\n{app_nonce}");
    to_hex(&hmac_sha256(token.as_bytes(), message.as_bytes()))
}

/// Proof the app sends in its `prove` frame.
pub fn app_proof(token: &str, bridge_nonce: &str, app_nonce: &str) -> String {
    proof("app", token, bridge_nonce, app_nonce)
}

/// Proof a command-line client (`benchup call --live`, a second MCP server)
/// sends in its `prove` frame.
pub fn client_proof(token: &str, bridge_nonce: &str, client_nonce: &str) -> String {
    proof("client", token, bridge_nonce, client_nonce)
}

/// Proof the bridge sends in its `ready` frame.
pub fn bridge_proof(token: &str, bridge_nonce: &str, app_nonce: &str) -> String {
    proof("bridge", token, bridge_nonce, app_nonce)
}

/// Constant-time comparison of two proofs.
pub fn proofs_match(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    // Proof lengths are public (always 64 hex characters), so comparing
    // them directly leaks nothing.
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

/// A nonce is exactly 64 lowercase hex characters.
pub fn valid_nonce(nonce: &str) -> bool {
    nonce.len() == 64 && nonce.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4231_vectors() {
        assert_eq!(
            to_hex(&hmac_sha256(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            to_hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        assert_eq!(
            to_hex(&hmac_sha256(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn proofs_are_role_separated() {
        let n = "a".repeat(64);
        let m = "b".repeat(64);
        assert_ne!(app_proof("t", &n, &m), bridge_proof("t", &n, &m));
        assert_ne!(app_proof("t", &n, &m), client_proof("t", &n, &m));
        assert!(proofs_match(&app_proof("t", &n, &m), &app_proof("t", &n, &m)));
        assert!(!proofs_match(&app_proof("t", &n, &m), &app_proof("u", &n, &m)));
        assert!(!proofs_match("ab", "abc"));
        // Lengths that differ by a multiple of 256, padded with NULs.
        let proof = app_proof("t", &n, &m);
        let padded = format!("{proof}{}", "\0".repeat(256));
        assert!(!proofs_match(&padded, &proof));
        assert!(!proofs_match(&proof, &padded));
    }

    #[test]
    fn nonce_validation() {
        assert!(valid_nonce(&"0f".repeat(32)));
        assert!(!valid_nonce(&"0F".repeat(32)));
        assert!(!valid_nonce("abc"));
        assert!(!valid_nonce(&"g".repeat(64)));
    }
}
