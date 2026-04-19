// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Auth — ed25519 + X25519 handshake, session table, authorized_keys management.

use parking_lot::RwLock;
use std::collections::HashMap;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand_core::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519Public};

pub struct Session {
    pub label: String,
    pub cipher: ChaCha20Poly1305,
    pub nonce_counter: u64,
}

pub struct SessionTable {
    sessions: RwLock<HashMap<[u8; 16], Session>>,
    max_sessions: usize,
}

pub fn create_session_table(max_sessions: usize) -> SessionTable {
    SessionTable {
        sessions: RwLock::new(HashMap::new()),
        max_sessions,
    }
}

pub struct AuthorizedKey {
    pub label: String,
    pub verifying_key: VerifyingKey,
}

pub fn parse_authorized_keys(content: &str) -> Vec<AuthorizedKey> {
    content
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let label = parts.next()?.to_string();
            let key_b64 = parts.next()?;
            let key_bytes = decode_base64(key_b64)?;
            if key_bytes.len() != 32 {
                return None;
            }
            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&key_bytes);
            let verifying_key = VerifyingKey::from_bytes(&key_array).ok()?;
            Some(AuthorizedKey {
                label,
                verifying_key,
            })
        })
        .collect()
}

pub struct HandshakeState {
    pub nonce: [u8; 32],
    pub server_secret: EphemeralSecret,
    pub server_public: X25519Public,
}

pub fn create_handshake() -> HandshakeState {
    let mut nonce = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut nonce);
    let server_secret = EphemeralSecret::random_from_rng(OsRng);
    let server_public = X25519Public::from(&server_secret);
    HandshakeState {
        nonce,
        server_secret,
        server_public,
    }
}

pub fn check_auth_proof(
    nonce: &[u8; 32],
    signature_bytes: &[u8; 64],
    verifying_key: &VerifyingKey,
) -> bool {
    let signature = Signature::from_bytes(signature_bytes);
    verifying_key.verify(nonce, &signature).is_ok()
}

pub fn derive_session_key(server_secret: EphemeralSecret, client_x25519_public: &[u8; 32]) -> Key {
    let client_public = X25519Public::from(*client_x25519_public);
    let shared_secret = server_secret.diffie_hellman(&client_public);
    *Key::from_slice(shared_secret.as_bytes())
}

impl SessionTable {
    pub fn create_session(&self, session_id: [u8; 16], label: String, key: Key) -> bool {
        let mut sessions = self.sessions.write();
        if sessions.len() >= self.max_sessions {
            return false;
        }
        let cipher = ChaCha20Poly1305::new(&key);
        sessions.insert(
            session_id,
            Session {
                label,
                cipher,
                nonce_counter: 0,
            },
        );
        true
    }

    pub fn encrypt_response(&self, session_id: &[u8; 16], plaintext: &[u8]) -> Option<Vec<u8>> {
        let mut sessions = self.sessions.write();
        let session = sessions.get_mut(session_id)?;
        session.nonce_counter += 1;
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..].copy_from_slice(&session.nonce_counter.to_be_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);
        session.cipher.encrypt(nonce, plaintext).ok()
    }

    pub fn decrypt_request(
        &self,
        session_id: &[u8; 16],
        ciphertext: &[u8],
        nonce_bytes: &[u8; 12],
    ) -> Option<Vec<u8>> {
        let sessions = self.sessions.read();
        let session = sessions.get(session_id)?;
        let nonce = Nonce::from_slice(nonce_bytes);
        session.cipher.decrypt(nonce, ciphertext).ok()
    }

    pub fn remove_session(&self, session_id: &[u8; 16]) {
        let mut sessions = self.sessions.write();
        sessions.remove(session_id);
    }

    pub fn read_session_count(&self) -> usize {
        self.sessions.read().len()
    }
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = table.iter().position(|&character| character == byte)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(output)
}

use rand_core::RngCore;
