use crate::crypto::srp::SrpClient;
use crate::error::{AirPlayError, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha512;
use std::collections::HashMap;

pub mod tlv {
    pub const METHOD: u8 = 0x00;
    pub const IDENTIFIER: u8 = 0x01;
    pub const SALT: u8 = 0x02;
    pub const PUBLIC_KEY: u8 = 0x03;
    pub const PROOF: u8 = 0x04;
    pub const ENCRYPTED_DATA: u8 = 0x05;
    pub const STATE: u8 = 0x06;
    pub const ERROR: u8 = 0x07;
    pub const SIGNATURE: u8 = 0x0A;
}

pub struct Tlv8;

impl Tlv8 {
    pub fn encode(items: &[(u8, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for &(tag, val) in items {
            let mut remaining = val;
            loop {
                let chunk_len = remaining.len().min(255);
                out.push(tag);
                out.push(chunk_len as u8);
                out.extend_from_slice(&remaining[..chunk_len]);
                remaining = &remaining[chunk_len..];
                if remaining.is_empty() {
                    break;
                }
            }
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> HashMap<u8, Vec<u8>> {
        let mut map = HashMap::new();
        let mut i = 0;
        while i + 1 < bytes.len() {
            let tag = bytes[i];
            let len = bytes[i + 1] as usize;
            i += 2;
            if i + len > bytes.len() {
                break;
            }
            let chunk = &bytes[i..i + len];
            map.entry(tag).or_insert_with(Vec::new).extend_from_slice(chunk);
            i += len;
        }
        map
    }
}

pub struct HomeKitPairing {
    srp_client: SrpClient,
    pub session_key: Option<[u8; 32]>,
}

impl HomeKitPairing {
    pub fn new(pin: &str) -> Self {
        Self {
            srp_client: SrpClient::new("Pair-Setup", pin),
            session_key: None,
        }
    }

    /// Pair-Setup M1 paketi
    pub fn create_setup_m1() -> Vec<u8> {
        Tlv8::encode(&[
            (tlv::STATE, &[0x01]),
            (tlv::METHOD, &[0x00]), // Pair-Setup
        ])
    }

    /// Pair-Setup M2 yanıtını işler ve M3 (A public key ve M1 proof) üretir
    pub fn handle_setup_m2(&mut self, m2_bytes: &[u8]) -> Result<Vec<u8>> {
        let tlv = Tlv8::decode(m2_bytes);
        let salt = tlv
            .get(&tlv::SALT)
            .ok_or_else(|| AirPlayError::Pairing("M2'de Salt bulunamadı".to_string()))?;
        let b_pub = tlv
            .get(&tlv::PUBLIC_KEY)
            .ok_or_else(|| AirPlayError::Pairing("M2'de B public key bulunamadı".to_string()))?;

        let m1_proof = self.srp_client.compute_m1(salt, b_pub)?;

        let m3 = Tlv8::encode(&[
            (tlv::STATE, &[0x03]),
            (tlv::PUBLIC_KEY, &self.srp_client.a_public_bytes),
            (tlv::PROOF, &m1_proof),
        ]);

        Ok(m3)
    }

    /// Pair-Setup M4 yanıtını (M2 proof) doğrular
    pub fn handle_setup_m4(&mut self, m4_bytes: &[u8]) -> Result<()> {
        let tlv = Tlv8::decode(m4_bytes);
        if let Some(err) = tlv.get(&tlv::ERROR) {
            return Err(AirPlayError::Pairing(format!("HomePod eşleşme hatası kodu: {:?}", err)));
        }

        let m2_proof = tlv
            .get(&tlv::PROOF)
            .ok_or_else(|| AirPlayError::Pairing("M4'te M2 proof bulunamadı".to_string()))?;

        self.srp_client.verify_m2(m2_proof)?;

        if let Some(k) = self.srp_client.session_key() {
            let hk = Hkdf::<Sha512>::new(Some(b"Pair-Setup-Encrypt-Salt"), k);
            let mut okm = [0u8; 32];
            hk.expand(b"Pair-Setup-Encrypt-Info", &mut okm)
                .map_err(|e| AirPlayError::Crypto(e.to_string()))?;
            self.session_key = Some(okm);
        }

        Ok(())
    }
}
