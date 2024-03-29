use std::fmt::Display;
use std::ops::Deref;

use primitive_types::U256;
use secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
use secp256k1::rand::rngs::OsRng;
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use tiny_keccak::Hasher;

use crate::common::Name;

// Hash
// ====

/// 256-bits hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hash(pub [u8; 32]);

impl Deref for Hash {
  type Target = [u8; 32];
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<&Hash> for U256 {
  fn from(value: &Hash) -> Self {
    U256::from_little_endian(&value.0)
  }
}

// Keccak256
// ---------

impl Hash {
  pub fn keccak256_from_bytes(data: &[u8]) -> Hash {
    let mut hasher = tiny_keccak::Keccak::v256();
    let mut output = [0u8; 32];
    hasher.update(data);
    hasher.finalize(&mut output);
    Hash(output)
  }
}

/// Can be hashed with Keccak256.
pub trait Keccakable {
  fn keccak256(&self) -> Hash;

  fn hashed(self) -> Hashed<Self>
  where
    Self: Sized,
  {
    Hashed::from(self)
  }
}

// Hashed
// ------

/// Wrapper that caches the Hash of some value
#[derive(Debug, Clone)]
pub struct Hashed<T> {
  data: T,
  hash: Hash,
}

impl<T> Hashed<T> {
  pub fn take(self) -> T {
    self.data
  }
  pub fn get_hash(&self) -> &Hash {
    &self.hash
  }
}

impl<T> Deref for Hashed<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.data
  }
}

impl<T: Keccakable> From<T> for Hashed<T> {
  fn from(data: T) -> Self {
    let hash = data.keccak256();
    Hashed { data, hash }
  }
}

impl<T> PartialEq for Hashed<T> {
  fn eq(&self, other: &Self) -> bool {
    self.hash == other.hash
  }
}

impl<T> Keccakable for Hashed<T> {
  fn keccak256(&self) -> Hash {
    self.get_hash().clone()
  }
}

// Address
// =======

/// Ethereum address
pub struct Address(pub [u8; 20]);

impl Address {
  pub fn from_public_key(pubk: &PublicKey) -> Self {
    Address::from_hash(&Account::hash_public_key(pubk))
  }

  pub fn from_hash(hash: &Hash) -> Self {
    Address(hash.0[12..32].try_into().unwrap())
  }

  pub fn show(&self) -> String {
    format!("0x{}", hex::encode(self.0))
  }
}

// Account
// =======

pub struct Account {
  secret_key: SecretKey,
  pub public_key: PublicKey,
  pub address: Address,
  pub name: Name,
}

impl Account {
  pub fn generate() -> Account {
    let secret_key = SecretKey::new(&mut OsRng::new().expect("OsRng"));
    Account::from_secret_key(secret_key)
  }

  pub fn hash_public_key(pubk: &PublicKey) -> Hash {
    let pubk_bytes = &pubk.serialize_uncompressed()[1..65];
    Hash::keccak256_from_bytes(pubk_bytes)
  }

  pub fn from_private_key(key: &[u8; 32]) -> Self {
    Account::from_secret_key(
      SecretKey::from_slice(key).expect("32 bytes private key"),
    )
  }

  pub fn from_secret_key(secret_key: SecretKey) -> Self {
    let pubk = PublicKey::from_secret_key(&Secp256k1::new(), &secret_key);
    let hash = Account::hash_public_key(&pubk);
    let addr = Address::from_hash(&hash);
    let name = Name::from_hash(&hash);
    Account { secret_key, public_key: pubk, address: addr, name }
  }

  pub fn sign(&self, hash: &Hash) -> Signature {
    let secp = Secp256k1::new();
    let msg = &Message::from_slice(&hash.0).expect("32 bytes hash");
    let sign =
      secp.sign_ecdsa_recoverable(msg, &self.secret_key).serialize_compact();
    Signature(
      [vec![sign.0.to_i32() as u8], sign.1.to_vec()]
        .concat()
        .try_into()
        .unwrap(),
    )
  }
}

// Signature
// =========

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "&str")]
pub struct Signature(pub [u8; 65]);

impl Signature {
  pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
    Some(Signature(bytes.try_into().ok()?))
  }

  pub fn from_hex(hex: &str) -> Option<Self> {
    Signature::from_bytes(hex::decode(hex).ok()?.as_slice())
  }

  pub fn to_hex(&self) -> String {
    hex::encode(self.0)
  }

  pub fn signer_public_key(&self, hash: &Hash) -> Option<PublicKey> {
    let recovery_id = RecoveryId::from_i32(self.0[0] as i32).ok()?;
    let sign_data = self.0[1..65].try_into().unwrap();
    let signature =
      RecoverableSignature::from_compact(sign_data, recovery_id).ok()?;
    signature
      .recover(&Message::from_slice(&hash.0).expect("32 bytes hash"))
      .ok()
  }

  pub fn signer_address(&self, hash: &Hash) -> Option<Address> {
    Some(Address::from_public_key(&self.signer_public_key(hash)?))
  }

  pub fn signer_name(&self, hash: &Hash) -> Option<Name> {
    Some(Name::from_public_key(&self.signer_public_key(hash)?))
  }
}

impl Display for Signature {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{}", self.to_hex()))
  }
}

impl From<Signature> for String {
  fn from(signature: Signature) -> Self {
    signature.to_hex()
  }
}

impl TryFrom<&str> for Signature {
  type Error = String;
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Signature::from_hex(value)
      .ok_or_else(|| "Invalid signature hex string".to_string())
  }
}

// Name
// ====

impl Name {
  pub fn from_public_key(pubk: &PublicKey) -> Self {
    Name::from_hash(&Account::hash_public_key(pubk))
  }

  // A Kindelia name is the first 120 bits of an Ethereum address.
  // This corresponds to the bytes 12-27 of the ECDSA public key.
  pub fn from_hash(hash: &Hash) -> Self {
    let bytes =
      vec![hash.0[12..27].to_vec(), vec![0]].concat().try_into().unwrap();
    Name::from_u128_unchecked(u128::from_be_bytes(bytes) >> 8)
  }
}

// ==== //

// TODO: remove or transform into a test
pub fn main() {
  // Creates an account from a private key
  let private = hex::decode(
    "0000000000000000000000000000000000000000000000000000000000000001",
  )
  .unwrap();
  let private = private.try_into().unwrap();
  let account = Account::from_private_key(&private);
  println!("addr: {}", hex::encode(account.address.0));

  // A message to sign
  let hash = Hash::keccak256_from_bytes(b"Hello!");

  // The signature
  let sign = account.sign(&hash);
  println!("sign: {}", hex::encode(sign.0));

  // Recovers the signer
  let auth = sign.signer_address(&hash).unwrap();
  println!("addr: {}", hex::encode(auth.0));

  // The signature, again
  let sign = Signature::from_hex("00d0bd2749ab84ce3851b4a28dd7f3b3e5a51ba6c38f36ef6e35fd0bd01c4a9d3418af687271eff0a37ed95e6a202f5d4efdb8663b361f301d899b3e5596313245").unwrap();
  let auth = sign.signer_address(&hash).unwrap();
  println!("addr: {}", hex::encode(auth.0));

  let name = Name::from_public_key(&account.public_key);
  println!("name: {}", name.show_hex());
}
