use sha2::{Digest, Sha256};

use crate::command::CommandStatusView;

use super::types::{ApiError, ChangedSubject, LobbyCommandResponse};

pub(super) fn changed(
    subject_kind: &str,
    subject_id_text: &str,
    operation: &str,
) -> ChangedSubject {
    ChangedSubject {
        subject_kind: subject_kind.to_string(),
        subject_id_text: subject_id_text.to_string(),
        operation: operation.to_string(),
    }
}

pub(super) fn duplicate_nonce_error(client_nonce: &str) -> ApiError {
    ApiError::new(
        "duplicate_nonce_payload_mismatch",
        format!("client nonce {client_nonce} was reused with a different payload"),
        false,
    )
}

pub(super) fn map_update_error(code: &str, error: impl ToString) -> ApiError {
    let message = error.to_string();
    let mapped = if message.contains("duplicate") && message.contains("payload") {
        "duplicate_nonce_payload_mismatch"
    } else if message.contains("payload too large") {
        "payload_too_large"
    } else if message.contains("retention limit") {
        "retention_limit_exceeded"
    } else if message.contains("events per turn limit") {
        "events_per_turn_limit_exceeded"
    } else if message.contains("not visible") || message.contains("hidden") {
        "not_visible"
    } else if message.contains("session is not active") {
        "session_not_active"
    } else if message.contains("insufficient") || message.contains("afford") {
        "insufficient_resources"
    } else if message.contains("budget") && message.contains("battle") {
        "battle_sync_incomplete"
    } else if message.contains("budget") {
        "recovery_budget_exhausted"
    } else {
        code
    };
    ApiError::new(mapped, message, false)
}

pub(super) fn lobby_status_view(response: &LobbyCommandResponse) -> CommandStatusView {
    CommandStatusView {
        command_id: response.command_id.clone(),
        status: response.status,
        phase: response.phase,
        retryable: response.retryable,
        error_code: response.error.as_ref().map(|error| error.code.clone()),
        error_message: response.error.as_ref().map(|error| error.message.clone()),
        result_json: None,
    }
}

pub(super) fn fallback_command_id(scope: &str, command_type: &str, client_nonce: &str) -> String {
    format!(
        "command:api:{scope}:{command_type}:{}",
        short_hash(client_nonce)
    )
}

pub(super) fn payload_hash(
    command_type: &str,
    actor_key: &str,
    client_nonce: &str,
    payload: &str,
) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "command_type", command_type);
    hash_text(&mut hasher, "actor_key", actor_key);
    hash_text(&mut hasher, "client_nonce", client_nonce);
    hash_text(&mut hasher, "payload", payload);
    to_hex(&hasher.finalize())
}

pub(super) fn nonce_u64(command_type: &str, client_nonce: &str) -> u64 {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "command_type", command_type);
    hash_text(&mut hasher, "client_nonce", client_nonce);
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}

pub(super) fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn short_hash(text: &str) -> String {
    payload_hash("short", "api", text, "")
        .chars()
        .take(16)
        .collect()
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update(value.len().to_be_bytes());
    hasher.update(value.as_bytes());
    hasher.update([0xFF]);
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0F) as usize] as char);
    }
    output
}
