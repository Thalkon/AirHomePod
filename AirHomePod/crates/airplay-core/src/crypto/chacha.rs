use crate::error::{AirPlayError, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Nonce,
};

pub struct ChachaSession {
    cipher_in: ChaCha20Poly1305,
    cipher_out: ChaCha20Poly1305,
    in_seq: u64,
    out_seq: u64,
}

impl ChachaSession {
    pub fn new(key_in: &[u8; 32], key_out: &[u8; 32]) -> Self {
        Self {
            cipher_in: ChaCha20Poly1305::new_from_slice(key_in).expect("Valid 32-byte key"),
            cipher_out: ChaCha20Poly1305::new_from_slice(key_out).expect("Valid 32-byte key"),
            in_seq: 0,
            out_seq: 0,
        }
    }

    fn make_nonce(seq: u64) -> Nonce {
        let mut nonce = [0u8; 12];
        nonce[4..12].copy_from_slice(&seq.to_le_bytes());
        *Nonce::from_slice(&nonce)
    }

    /// Şifrelenmiş HAP/RTSP çerçevesi oluşturur (2-byte uzunluk + şifreli veri + 16-byte tag)
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Self::make_nonce(self.out_seq);
        self.out_seq += 1;

        let len_bytes = (plaintext.len() as u16).to_le_bytes();
        let payload = Payload {
            msg: plaintext,
            aad: &len_bytes,
        };

        let ciphertext = self
            .cipher_out
            .encrypt(&nonce, payload)
            .map_err(|e| AirPlayError::Crypto(format!("ChaCha şifreleme hatası: {:?}", e)))?;

        let mut packet = Vec::with_capacity(2 + ciphertext.len());
        packet.extend_from_slice(&len_bytes);
        packet.extend_from_slice(&ciphertext);
        Ok(packet)
    }

    /// Şifreli çerçeveyi çözer
    pub fn decrypt(&mut self, len_bytes: [u8; 2], ciphertext_with_tag: &[u8]) -> Result<Vec<u8>> {
        let nonce = Self::make_nonce(self.in_seq);
        self.in_seq += 1;

        let payload = Payload {
            msg: ciphertext_with_tag,
            aad: &len_bytes,
        };

        self.cipher_in
            .decrypt(&nonce, payload)
            .map_err(|e| AirPlayError::Crypto(format!("ChaCha şifre çözme hatası: {:?}", e)))
    }
}
