#![feature(external_doc)]
#![doc(include = "../README.md")]
#![no_std]
#![allow(non_snake_case)]

#[cfg(all(feature = "alloc", not(feature = "std")))]
#[macro_use]
extern crate alloc;

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

use fun::{derive_nonce, g, marker::*, nonce::NonceGen, s, Point, Scalar, G};
pub use secp256kfun as fun;
pub use secp256kfun::nonce;
mod signature;
pub use signature::Signature;
pub mod adaptor;

/// An instance of the ECDSA signature scheme.
#[derive(Default, Clone)]
pub struct ECDSA<NG> {
    /// An instance of [`NonceGen`] to produce nonces.
    ///
    /// [`NonceGen`]: crate::nonce::NonceGen
    pub nonce_gen: NG,
    /// `enforce_low_s`: Whether the verify algorithm should enforce that the `s` component of the signature is low (see [BIP-146]).
    ///
    /// [BIP-146]: https://github.com/bitcoin/bips/blob/master/bip-0146.mediawiki#low_s
    pub enforce_low_s: bool,
}

impl ECDSA<()> {
    /// Creates an `ECDSA` instance that cannot be used to sign messages but can
    /// verify signatures.
    pub fn verify_only() -> Self {
        ECDSA {
            nonce_gen: (),
            enforce_low_s: false,
        }
    }
}

impl<NG> ECDSA<NG> {
    /// Transforms the ECDSA instance into one which enforces the [BIP-146] low s constraint.
    ///
    /// [BIP-146]: https://github.com/bitcoin/bips/blob/master/bip-0146.mediawiki#low_s
    pub fn enforce_low_s(self) -> Self {
        ECDSA {
            nonce_gen: self.nonce_gen,
            enforce_low_s: true,
        }
    }
}

impl<NG> ECDSA<NG> {
    /// Verify an ECDSA signature.
    pub fn verify(
        &self,
        verification_key: &Point<impl PointType, Public, NonZero>,
        message: &[u8; 32],
        signature: &Signature<impl Secrecy>,
    ) -> bool {
        let (R_x, s) = signature.as_tuple();
        // This ensures that there is only one valid s value per R_x for any given message.
        if s.is_high() && self.enforce_low_s {
            return false;
        }

        let m = Scalar::from_bytes_mod_order(message.clone()).mark::<Public>();
        let s_inv = s.invert();

        g!((s_inv * m) * G + (s_inv * R_x) * verification_key)
            .mark::<NonZero>()
            .map_or(false, |implied_R| implied_R.x_eq_scalar(R_x))
    }
}

impl<NG: NonceGen> ECDSA<NG> {
    /// Creates a ECDSA instance.
    ///
    /// The caller chooses how nonces are generated by providing a [`NonceGen`].
    ///
    /// # Example
    /// ```
    /// use ecdsa_fun::{nonce, ECDSA};
    /// use rand::rngs::ThreadRng;
    /// use sha2::Sha256;
    /// let nonce_gen = nonce::from_global_rng::<Sha256, ThreadRng>();
    /// let ecdsa = ECDSA::new(nonce_gen);
    /// ```
    ///
    /// [`NonceGen`]: crate::nonce::NonceGen
    pub fn new(nonce_gen: NG) -> Self {
        ECDSA {
            nonce_gen: nonce_gen.add_protocol_tag("ECDSA"),
            enforce_low_s: false,
        }
    }

    /// Deterministically produce a ECDSA signature on a message hash.
    ///
    /// # Examples
    ///
    /// ```
    /// use ecdsa_fun::{
    ///     fun::{digest::Digest, g, marker::*, Scalar, G},
    ///     nonce, ECDSA,
    /// };
    /// use rand::rngs::ThreadRng;
    /// use sha2::Sha256;
    /// let secret_key = Scalar::random(&mut rand::thread_rng());
    /// let public_key = g!(secret_key * G).mark::<Normal>();
    /// let ecdsa = ECDSA::new(nonce::from_global_rng::<Sha256, ThreadRng>());
    /// let message = b"Attack at dawn";
    /// let message_hash = {
    ///     let mut message_hash = [0u8; 32];
    ///     let hash = Sha256::default().chain(message);
    ///     message_hash.copy_from_slice(hash.finalize().as_ref());
    ///     message_hash
    /// };
    /// let signature = ecdsa.sign(&secret_key, &message_hash);
    /// assert!(ecdsa.verify(&public_key, &message_hash, &signature));
    /// ```
    pub fn sign(&self, secret_key: &Scalar, message_hash: &[u8; 32]) -> Signature {
        let x = secret_key;
        let m = Scalar::from_bytes_mod_order(message_hash.clone()).mark::<Public>();
        let r = derive_nonce!(
            nonce_gen => self.nonce_gen,
            secret => x,
            public => [&message_hash[..]]
        );
        let R = g!(r * G).mark::<Normal>(); // Must be normal so we can get x-coordinate

        // This coverts R is its x-coordinate mod q. This acts as a kind of poor
        // man's version of the Fiat-Shamir challenge in a Schnorr
        // signature. The lack of any known algebraic relationship between r and
        // R_x is what makes ECDSA signatures difficult to forge.
        let R_x = Scalar::from_bytes_mod_order(R.to_xonly().into_bytes())
            // There *is* a single point that will be zero here but since we're
            // choosing R pseudorandomly it won't occur.
            .mark::<(Public, NonZero)>()
            .expect("computationally unreachable");

        let mut s = s!({ r.invert() } * (m + R_x * x))
            // Given R_x is determined by x and m through a hash, reaching
            // (m + R_x * x) = 0 is intractable.
            .mark::<NonZero>()
            .expect("computationally unreachable");

        // s values must be low (less than half group order), otherwise signatures
        // would be malleable i.e. (R,s) and (R,-s) would both be valid signatures.
        s.conditional_negate(s.is_high());

        Signature {
            R_x,
            s: s.mark::<Public>(),
        }
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! test_instance {
    () => {
        $crate::ECDSA::new($crate::nonce::Deterministic::<sha2::Sha256>::default())
    };
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::RngCore;
    use secp256kfun::TEST_SOUNDNESS;

    #[test]
    fn repeated_sign_and_verify() {
        let ecdsa = test_instance!();
        for _ in 0..20 {
            let mut message = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut message);
            let secret_key = Scalar::random(&mut rand::thread_rng());
            let public_key = g!(secret_key * G).mark::<Normal>();
            let sig = ecdsa.sign(&secret_key, &message);
            assert!(ecdsa.verify(&public_key, &message, &sig))
        }
    }

    #[test]
    fn low_s() {
        let ecdsa_enforce_low_s = test_instance!().enforce_low_s();
        let ecdsa = test_instance!();
        for _ in 0..TEST_SOUNDNESS {
            let mut message = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut message);
            let secret_key = Scalar::random(&mut rand::thread_rng());
            let public_key = g!(secret_key * G);
            let mut sig = ecdsa.sign(&secret_key, &message);
            assert!(ecdsa.verify(&public_key, &message, &sig));
            assert!(ecdsa_enforce_low_s.verify(&public_key, &message, &sig));
            sig.s = -sig.s;
            assert!(!ecdsa_enforce_low_s.verify(&public_key, &message, &sig));
            assert!(ecdsa.verify(&public_key, &message, &sig));
        }
    }
}
