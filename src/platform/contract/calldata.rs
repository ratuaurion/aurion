//! Encoder calldata AVM kanonikal untuk Contract SDK.
//!
//! Karena `BytecodeVerifier` (AUR-VM-005) mewajibkan **setiap byte payload berupa
//! opcode yang valid**, calldata mentah `selector || args` tidak dapat langsung
//! ditransmisikan. Konvensi resmi yang diterapkan modul ini adalah **AVM Call Frame**:
//!
//! ```text
//! payload := PUSH32 argN ... PUSH32 arg1  PUSH4 selector  <runtime script>
//!            \___ argumen (urutan terbalik) ___/  \___ top ___/
//! ```
//!
//! - Stack (atas -> bawah): `selector, arg1, arg2, ... argN` — muat di jendela
//!   instruksi `DUP4`/`SWAP4` (maksimal 4 argumen, `MAX_ABI_INPUTS`).
//! - Target `JUMP` absolut milik runtime **direlokasi statis** mengikuti panjang
//!   frame (pola terbukti `PUSHn imm; JUMP|JUMPI`) karena `Opcode::Pc` hanyalah
//!   penanda no-op pada AVM.
//! - Seluruh payload wajib lulus `BytecodeVerifier` **sebelum** signing.

use crate::transaction::types::MAX_TRANSACTION_PAYLOAD_BYTES;
use crate::vm::opcode::Opcode;
use crate::vm::verifier::{BytecodeVerifier, MAX_BYTECODE_SIZE};

use super::error::ContractError;
use super::metadata::{AbiValue, MethodAbi};

/// Panjang immediate PUSH per opcode (0 bila bukan PUSH).
fn push_immediate_len(op: Opcode) -> usize {
    match op {
        Opcode::Push1 => 1,
        Opcode::Push2 => 2,
        Opcode::Push4 => 4,
        Opcode::Push8 => 8,
        Opcode::Push16 => 16,
        Opcode::Push32 => 32,
        _ => 0,
    }
}

/// Susun call frame: argumen (terbalik) di-push lebih dulu, selector terakhir
/// sehingga selector berada di puncak stack saat runtime mulai dieksekusi.
///
/// # Errors
/// Jumlah/tipe argumen tidak sesuai deklarasi metode atau melebihi jendela stack.
pub fn encode_call_frame(method: &MethodAbi, args: &[AbiValue]) -> Result<Vec<u8>, ContractError> {
    method.check_args(args)?;
    let mut frame = Vec::with_capacity(args.len() * 33 + 5);
    for value in args.iter().rev() {
        frame.push(Opcode::Push32 as u8);
        frame.extend_from_slice(&value.encode_word());
    }
    frame.push(Opcode::Push4 as u8);
    frame.extend_from_slice(&method.selector);
    Ok(frame)
}

/// Relokasi statis target `JUMP`/`JUMPI` absolut dalam `code` sebesar `offset`.
///
/// Hanya pola yang **terbukti statis** (`PUSHn imm` langsung diikuti `JUMP|JUMPI`)
/// yang diubah; nilai yang tidak muat pada lebar immediate atau pola komputasi
/// lain dibiarkan apa adanya dan akan ditangkap gerbang dry-run sebelum signing.
#[must_use]
pub fn relocate_jumps(code: &[u8], offset: usize) -> Vec<u8> {
    let mut out = code.to_vec();
    if offset == 0 || out.is_empty() {
        return out;
    }
    let mut pc = 0usize;
    // (posisi_awal_imm, panjang_imm) dari PUSH tepat sebelumnya.
    let mut pending: Option<(usize, usize)> = None;
    while pc < out.len() {
        let Ok(op) = Opcode::from_u8(out[pc]) else {
            break; // opcode tidak valid akan ditolak BytecodeVerifier
        };
        let imm_len = push_immediate_len(op);
        match op {
            Opcode::Jump | Opcode::Jumpi => {
                if let Some((start, len)) = pending.take() {
                    if len <= 8 && start + len <= out.len() {
                        let mut target_bytes = [0u8; 8];
                        target_bytes[8 - len..].copy_from_slice(&out[start..start + len]);
                        if let Some(new_target) =
                            u64::from_be_bytes(target_bytes).checked_add(offset as u64)
                        {
                            let encoded = new_target.to_be_bytes();
                            // Hanya tulis ulang bila nilai tetap muat pada lebar immediate.
                            if encoded[..8 - len].iter().all(|&b| b == 0) {
                                out[start..start + len].copy_from_slice(&encoded[8 - len..]);
                            }
                        }
                    }
                }
                pending = None;
            }
            _ if imm_len > 0 => {
                pending = Some((pc + 1, imm_len));
            }
            _ => pending = None,
        }
        pc += 1 + imm_len;
    }
    out
}

/// Susun payload `ContractCall` lengkap: call frame + runtime (direlokasi),
/// lalu wajib lulus verifikasi statis AVM.
///
/// # Errors
/// Argumen tidak valid, bytecode runtime/payload gagal verifikasi, atau payload
/// melebihi batas 24 KB.
pub fn encode_call_payload(
    method: &MethodAbi,
    args: &[AbiValue],
    runtime: &[u8],
) -> Result<Vec<u8>, ContractError> {
    let frame = encode_call_frame(method, args)?;
    let code = relocate_jumps(runtime, frame.len());
    let mut payload = frame;
    payload.extend_from_slice(&code);
    validate_payload(&payload)?;
    Ok(payload)
}

/// Susun payload `ContractDeploy` (konstruktor) dan wajibkan verifikasi statis.
///
/// # Errors
/// Konstruktor kosong / tidak valid / melebihi batas 24 KB.
pub fn encode_deploy_payload(constructor: &[u8]) -> Result<Vec<u8>, ContractError> {
    let payload = constructor.to_vec();
    validate_payload(&payload)?;
    Ok(payload)
}

/// Validasi gabungan: batas payload transaksi + `BytecodeVerifier` (AUR-VM-005).
///
/// # Errors
/// Payload melebihi batas atau mengandung opcode tidak valid.
pub fn validate_payload(payload: &[u8]) -> Result<(), ContractError> {
    if payload.len() > MAX_TRANSACTION_PAYLOAD_BYTES {
        return Err(ContractError::Verification(format!(
            "Payload {} byte melebihi batas {} byte",
            payload.len(),
            MAX_TRANSACTION_PAYLOAD_BYTES
        )));
    }
    if payload.len() > MAX_BYTECODE_SIZE {
        return Err(ContractError::Verification(format!(
            "Bytecode {} byte melebihi batas {} byte",
            payload.len(),
            MAX_BYTECODE_SIZE
        )));
    }
    BytecodeVerifier::verify(payload).map_err(|e| ContractError::Verification(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::contract::metadata::{AbiParam, AbiType};

    fn two_arg_method() -> MethodAbi {
        MethodAbi::new(
            "set",
            "set(u64,u32)",
            vec![
                AbiParam {
                    name: "a".to_string(),
                    ty: AbiType::U64,
                },
                AbiParam {
                    name: "b".to_string(),
                    ty: AbiType::U32,
                },
            ],
            vec![],
            false,
        )
        .expect("method")
    }

    #[test]
    fn test_relocate_jumps_shifts_direct_targets() {
        // PUSH1 4; JUMP; STOP; JUMPDEST(@4)
        let code = vec![
            Opcode::Push1 as u8,
            0x04,
            Opcode::Jump as u8,
            Opcode::Stop as u8,
            Opcode::JumpDest as u8,
        ];
        let relocated = relocate_jumps(&code, 6);
        assert_eq!(relocated[1], 0x0A, "target 4 + offset 6 = 10");
        validate_payload(&relocated).expect("relocated code harus tervalidasi");
    }

    #[test]
    fn test_relocate_skips_unfittable_targets() {
        // PUSH1 250; JUMP -> 250 + 6 = 256 tidak muat di 1 byte: dibiarkan.
        let code = vec![
            Opcode::Push1 as u8,
            250,
            Opcode::Jump as u8,
            Opcode::Stop as u8,
        ];
        let relocated = relocate_jumps(&code, 6);
        assert_eq!(relocated[1], 250);
    }

    #[test]
    fn test_call_frame_layout() {
        let method = two_arg_method();
        let args = vec![AbiValue::U64(7), AbiValue::U32(3)];
        let frame = encode_call_frame(&method, &args).expect("frame");
        // Urutan: PUSH32 arg2, PUSH32 arg1, PUSH4 selector
        assert_eq!(frame.len(), 1 + 32 + 1 + 32 + 1 + 4);
        assert_eq!(frame[0], Opcode::Push32 as u8);
        assert_eq!(frame[1 + 32], Opcode::Push32 as u8);
        let sel_at = 1 + 32 + 1 + 32;
        assert_eq!(frame[sel_at], Opcode::Push4 as u8);
        assert_eq!(&frame[sel_at + 1..], &method.selector);
    }

    #[test]
    fn test_encode_call_payload_relocates_and_verifies() {
        let method = two_arg_method();
        let args = vec![AbiValue::U64(7), AbiValue::U32(3)];
        // Runtime dengan jump absolut: PUSH1 4; JUMP; STOP; JUMPDEST
        let runtime = vec![
            Opcode::Push1 as u8,
            0x04,
            Opcode::Jump as u8,
            Opcode::Stop as u8,
            Opcode::JumpDest as u8,
        ];
        let frame_len = encode_call_frame(&method, &args).expect("frame").len();
        let payload = encode_call_payload(&method, &args, &runtime).expect("payload");
        assert_eq!(
            payload[frame_len + 1],
            (4 + frame_len) as u8,
            "target jump di-relokasi mengikuti panjang frame"
        );
        validate_payload(&payload).expect("payload tervalidasi");
    }

    #[test]
    fn test_wrong_arg_types_rejected() {
        let method = two_arg_method();
        let err = encode_call_frame(&method, &[AbiValue::U64(1)]).expect_err("harus gagal");
        assert!(matches!(err, ContractError::Abi(_)));
    }
}
