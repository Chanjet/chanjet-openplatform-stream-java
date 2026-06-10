use aes::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit as AesKeyInit};
use aes::Aes128;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, KeyInit as HmacKeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn hmac_sha256(data: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

pub fn sanitize_key(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            c != '\u{200b}'
                && c != '\u{200c}'
                && c != '\u{200d}'
                && c != '\u{feff}'
                && !c.is_control()
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn aes_decrypt(encrypted_base64: &str, decrypt_key: &str) -> Result<String> {
    let decrypt_key = sanitize_key(decrypt_key);
    let ciphertext = general_purpose::STANDARD
        .decode(encrypted_base64)
        .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;

    let key_bytes = if decrypt_key.len() == 32 {
        hex::decode(decrypt_key)
            .map_err(|e| anyhow!("Failed to decode 32-character decryption key as hex: {}", e))?
    } else {
        decrypt_key.as_bytes().to_vec()
    };

    if key_bytes.len() != 16 {
        return Err(anyhow!(
            "AES-128 key must be 16 bytes (or 32 hex characters), got {}",
            key_bytes.len()
        ));
    }

    let cipher = Aes128::new(GenericArray::from_slice(&key_bytes));
    let mut plaintext = ciphertext.clone();

    if plaintext.len() % 16 != 0 {
        return Err(anyhow!("Ciphertext length is not a multiple of 16"));
    }

    for chunk in plaintext.chunks_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }

    let unpadded = pkcs7_unpad(&plaintext, 16)?;
    String::from_utf8(unpadded.to_vec()).map_err(|e| anyhow!("UTF-8 decode failed: {}", e))
}

fn pkcs7_unpad(data: &[u8], block_size: usize) -> Result<&[u8]> {
    let len = data.len();
    if len == 0 {
        return Err(anyhow!("Data is empty"));
    }

    let padding_len = data[len - 1] as usize;
    if padding_len == 0 || padding_len > block_size || padding_len > len {
        return Err(anyhow!("Invalid padding length: {}", padding_len));
    }

    for i in 0..padding_len {
        if data[len - 1 - i] != padding_len as u8 {
            return Err(anyhow!("Invalid padding content"));
        }
    }

    Ok(&data[..len - padding_len])
}
