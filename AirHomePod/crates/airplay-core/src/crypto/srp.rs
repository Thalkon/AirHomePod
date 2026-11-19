use crate::error::{AirPlayError, Result};
use num_bigint::BigUint;
use rand::RngCore;
use sha2::{Digest, Sha512};

// RFC 5054 3072-bit SRP Group
const RFC5054_N_HEX: &str = "\
FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD1\
29024E088A67CC74020BBEA63B139B22514A08798E3404DD\
EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245\
E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED\
EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3D\
C2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F\
83655D23DCA3AD961C62F356208552BB9ED529077096966D\
670C354E4ABC9804F1746C08CA18217C32905E462E36CE3B\
E39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9\
DE2BCBF6955817183995497CEA956AE515D2261898FA0510\
15728E5A8AAAC42DAD33170D04507A33A85521ABDF1CBA64\
ECFB850458DBEF0A8AEA7157FFD9F045864385C3615CE942\
4E73D832FB749B6F01E1FF433624E2741B2ACF7996471F40\
CEE38B69E4C87799F454AE7F3DABA8FDF9FB269B9F40DE57\
22774298A5E4F83927FF0599F7C0C3E48B03BB52FECD376B\
2A70DA48F7729D95B453436830E3074D0594FE535A6F80D3\
19A4FF7A29A4D3380D41E7A8";

const RFC5054_G: u32 = 5;

pub struct SrpClient {
    n: BigUint,
    g: BigUint,
    k: BigUint,
    a_secret: BigUint,
    pub a_public_bytes: Vec<u8>,
    username: String,
    password: String,
    k_session: Option<Vec<u8>>,
    m1_proof: Option<Vec<u8>>,
}

impl SrpClient {
    pub fn new(username: &str, password: &str) -> Self {
        let n = BigUint::parse_bytes(RFC5054_N_HEX.as_bytes(), 16).expect("Valid N hex");
        let g = BigUint::from(RFC5054_G);

        // k = H(N || PAD(g))
        let n_bytes = n.to_bytes_be();
        let mut g_padded = vec![0u8; n_bytes.len()];
        let g_raw = g.to_bytes_be();
        g_padded[n_bytes.len() - g_raw.len()..].copy_from_slice(&g_raw);

        let mut hasher = Sha512::new();
        hasher.update(&n_bytes);
        hasher.update(&g_padded);
        let k_hash = hasher.finalize();
        let k = BigUint::from_bytes_be(&k_hash);

        // Generate ephemeral secret `a` (at least 256 bits)
        let mut rng = rand::thread_rng();
        let mut a_bytes = [0u8; 32];
        rng.fill_bytes(&mut a_bytes);
        let a_secret = BigUint::from_bytes_be(&a_bytes);

        // A = g^a mod N
        let a_public = g.modpow(&a_secret, &n);
        let a_pub_bytes = a_public.to_bytes_be();
        let mut a_padded = vec![0u8; n_bytes.len()];
        let offset = n_bytes.len().saturating_sub(a_pub_bytes.len());
        a_padded[offset..].copy_from_slice(&a_pub_bytes);

        Self {
            n,
            g,
            k,
            a_secret,
            a_public_bytes: a_padded,
            username: username.to_string(),
            password: password.to_string(),
            k_session: None,
            m1_proof: None,
        }
    }

    /// Processes salt and server public key B, computes M1 proof
    pub fn compute_m1(&mut self, salt: &[u8], b_bytes: &[u8]) -> Result<Vec<u8>> {
        let b = BigUint::from_bytes_be(b_bytes);
        if &b == &BigUint::from(0u32) || &b >= &self.n {
            return Err(AirPlayError::Pairing("Geçersiz sunucu SRP B anahtarı".to_string()));
        }

        // u = H(PAD(A) || PAD(B))
        let n_len = self.n.to_bytes_be().len();
        let mut b_padded = vec![0u8; n_len];
        if b_bytes.len() <= n_len {
            b_padded[n_len - b_bytes.len()..].copy_from_slice(b_bytes);
        } else {
            b_padded.copy_from_slice(&b_bytes[b_bytes.len() - n_len..]);
        }

        let mut u_hasher = Sha512::new();
        u_hasher.update(&self.a_public_bytes);
        u_hasher.update(&b_padded);
        let u_hash = u_hasher.finalize();
        let u = BigUint::from_bytes_be(&u_hash);

        // x = H(s || H(I || ":" || P))
        let mut id_hasher = Sha512::new();
        id_hasher.update(format!("{}:{}", self.username, self.password).as_bytes());
        let id_hash = id_hasher.finalize();

        let mut x_hasher = Sha512::new();
        x_hasher.update(salt);
        x_hasher.update(&id_hash);
        let x_hash = x_hasher.finalize();
        let x = BigUint::from_bytes_be(&x_hash);

        // S = (B - k * (g^x mod N)) ^ (a + u * x) mod N
        let gx = self.g.modpow(&x, &self.n);
        let kgx = (&self.k * &gx) % &self.n;

        let base = if &b >= &kgx {
            (&b - &kgx) % &self.n
        } else {
            (&self.n - (&kgx - &b) % &self.n) % &self.n
        };

        let exp = &self.a_secret + (&u * &x);
        let s = base.modpow(&exp, &self.n);
        let s_bytes = s.to_bytes_be();

        // K = H(S)
        let mut k_hasher = Sha512::new();
        k_hasher.update(&s_bytes);
        let k_hash = k_hasher.finalize().to_vec();

        // H(N) ^ H(g)
        let h_n = Sha512::digest(&self.n.to_bytes_be());
        let h_g = Sha512::digest(&self.g.to_bytes_be());
        let mut h_xor = [0u8; 64];
        for i in 0..64 {
            h_xor[i] = h_n[i] ^ h_g[i];
        }

        let h_i = Sha512::digest(self.username.as_bytes());

        // M1 = H(H_XOR || H(I) || s || A || B || K)
        let mut m1_hasher = Sha512::new();
        m1_hasher.update(&h_xor);
        m1_hasher.update(&h_i);
        m1_hasher.update(salt);
        m1_hasher.update(&self.a_public_bytes);
        m1_hasher.update(&b_padded);
        m1_hasher.update(&k_hash);
        let m1 = m1_hasher.finalize().to_vec();

        self.k_session = Some(k_hash);
        self.m1_proof = Some(m1.clone());

        Ok(m1)
    }

    /// Verifies server M2 proof: H(A || M1 || K)
    pub fn verify_m2(&self, m2_bytes: &[u8]) -> Result<()> {
        let k = self.k_session.as_ref().ok_or_else(|| {
            AirPlayError::Pairing("Oturum anahtarı henüz hesaplanmadı".to_string())
        })?;
        let m1 = self.m1_proof.as_ref().ok_or_else(|| {
            AirPlayError::Pairing("M1 kanıtı henüz hesaplanmadı".to_string())
        })?;

        let mut hasher = Sha512::new();
        hasher.update(&self.a_public_bytes);
        hasher.update(m1);
        hasher.update(k);
        let expected_m2 = hasher.finalize();

        if expected_m2.as_slice() == m2_bytes {
            Ok(())
        } else {
            Err(AirPlayError::Pairing("Sunucu M2 kanıt doğrulaması başarısız".to_string()))
        }
    }

    pub fn session_key(&self) -> Option<&[u8]> {
        self.k_session.as_deref()
    }
}
