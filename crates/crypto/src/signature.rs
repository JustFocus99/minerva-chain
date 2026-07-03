use crate::hash::hash_bytes;
use primitives::{PublicKeyBytes, SignatureBytes};

const DEFAULT_PUBLIC_KEY: PublicKeyBytes = [7u8; 32];

/// Create a deterministic placeholder signature over the provided bytes.
pub fn sign_message(message: &[u8]) -> (PublicKeyBytes, SignatureBytes) {
    let public_key = DEFAULT_PUBLIC_KEY;
    let hash = hash_bytes(message);
    let mut signature = [0u8; 64];
    signature[..32].copy_from_slice(&hash);
    signature[32..].copy_from_slice(&hash);
    (public_key, signature)
}

/// Verify a deterministic placeholder signature over the provided bytes.
pub fn verify_signature(
    message: &[u8],
    public_key: PublicKeyBytes,
    signature: SignatureBytes,
) -> bool {
    let expected = sign_message(message).1;
    signature == expected && public_key == DEFAULT_PUBLIC_KEY
}
