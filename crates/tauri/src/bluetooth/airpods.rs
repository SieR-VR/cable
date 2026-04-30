//! Apple Continuity advertisement parser (a.k.a. AppleCP / "Proximity Pairing"
//! packet, type 0x07). Pure logic — no Windows or std-time deps so it can be
//! unit-tested without a runtime.
//!
//! Layout of the 27-byte payload (after the company-id prefix has been
//! stripped by the OS):
//!
//! ```text
//!   off  size  field
//!   0    1     packetType     == 0x07
//!   1    1     remainingLength== 0x19 (25)
//!   2    1     unk1
//!   3-4  2     modelId        (LE)  e.g. AirPods Pro = 0x200E, Pro2 = 0x2014, Pro2 USB-C = 0x2024
//!   5    1     statusFlags    bit5 set => broadcastFromLeft
//!   6    1     batt0          low4 = curr,  high4 = anot   (0..10, 15 = unknown)
//!   7    1     batt1          low4 = case,  bit4 = currChg, bit5 = anotChg, bit6 = caseChg
//!   8    1     lid            switchCount(3) | closed(1)
//!   9    1     color
//!   10   1     unk11
//!   11-26 16   encrypted/hash payload (privacy)
//! ```
//!
//! Battery values 0..10 mean 0..100 % in 10 % steps, 15 means "unknown".

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplePayload {
  pub model_id: u16,
  pub broadcast_from_left: bool,
  pub left: Option<u8>,
  pub right: Option<u8>,
  pub case_: Option<u8>,
  pub charging_left: bool,
  pub charging_right: bool,
  pub charging_case: bool,
}

const APPLE_CP_PROXIMITY_PAIRING: u8 = 0x07;
const APPLE_CP_LEN: usize = 27;

/// Parse the AppleCP proximity-pairing payload. Returns `None` if the data
/// doesn't look like a valid 27-byte AppleCP packet of type 0x07.
pub fn parse_apple_continuity(data: &[u8]) -> Option<ApplePayload> {
  if data.len() != APPLE_CP_LEN || data[0] != APPLE_CP_PROXIMITY_PAIRING {
    return None;
  }
  let model_id = u16::from_le_bytes([data[3], data[4]]);
  let broadcast_from_left = (data[5] & 0b0010_0000) != 0;
  let batt0 = data[6];
  let batt1 = data[7];

  let curr_raw = batt0 & 0x0F;
  let anot_raw = batt0 >> 4;
  let case_raw = batt1 & 0x0F;
  let curr_chg = (batt1 & 0b0001_0000) != 0;
  let anot_chg = (batt1 & 0b0010_0000) != 0;
  let case_chg = (batt1 & 0b0100_0000) != 0;

  // "curr" is the bud doing the broadcasting, "anot" is the other one.
  let (left_raw, right_raw, charging_left, charging_right) = if broadcast_from_left {
    (curr_raw, anot_raw, curr_chg, anot_chg)
  } else {
    (anot_raw, curr_raw, anot_chg, curr_chg)
  };

  Some(ApplePayload {
    model_id,
    broadcast_from_left,
    left: raw_to_percent(left_raw),
    right: raw_to_percent(right_raw),
    case_: raw_to_percent(case_raw),
    charging_left,
    charging_right,
    charging_case: case_chg,
  })
}

/// Convert a 4-bit AppleCP battery field (0..10 = 0..100 %, 15 = unknown).
fn raw_to_percent(raw: u8) -> Option<u8> {
  match raw {
    0..=10 => Some(raw * 10),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make(payload: &[(usize, u8)]) -> [u8; 27] {
    let mut b = [0u8; 27];
    b[0] = APPLE_CP_PROXIMITY_PAIRING;
    b[1] = 0x19;
    for &(off, v) in payload {
      b[off] = v;
    }
    b
  }

  #[test]
  fn rejects_wrong_length() {
    assert!(parse_apple_continuity(&[0x07, 0x19]).is_none());
    assert!(parse_apple_continuity(&[0u8; 26]).is_none());
    assert!(parse_apple_continuity(&[0u8; 28]).is_none());
  }

  #[test]
  fn rejects_wrong_packet_type() {
    let mut b = [0u8; 27];
    b[0] = 0x05;
    assert!(parse_apple_continuity(&b).is_none());
  }

  #[test]
  fn parses_model_id_le() {
    // Pro 2 USB-C => 0x2024
    let b = make(&[(3, 0x24), (4, 0x20)]);
    let p = parse_apple_continuity(&b).unwrap();
    assert_eq!(p.model_id, 0x2024);
  }

  #[test]
  fn batteries_left_broadcast() {
    // broadcastFromLeft=1, curr=8 (left), anot=6 (right), case=4
    // batt0 = (anot << 4) | curr = (6 << 4) | 8 = 0x68
    // batt1 = case = 0x04, with bit4 (currChg=left), bit5 (anotChg=right), bit6 (caseChg) = 0x70
    let b = make(&[(5, 0b0010_0000), (6, 0x68), (7, 0x04 | 0x70)]);
    let p = parse_apple_continuity(&b).unwrap();
    assert!(p.broadcast_from_left);
    assert_eq!(p.left, Some(80));
    assert_eq!(p.right, Some(60));
    assert_eq!(p.case_, Some(40));
    assert!(p.charging_left);
    assert!(p.charging_right);
    assert!(p.charging_case);
  }

  #[test]
  fn batteries_right_broadcast() {
    // broadcastFromLeft=0, curr=right=3, anot=left=5, case=15(unknown)
    // batt0 = (anot << 4) | curr = (5 << 4) | 3 = 0x53
    // batt1 = case=0x0F, bit4=currChg(right)
    let b = make(&[(5, 0), (6, 0x53), (7, 0x0F | 0x10)]);
    let p = parse_apple_continuity(&b).unwrap();
    assert!(!p.broadcast_from_left);
    assert_eq!(p.left, Some(50));
    assert_eq!(p.right, Some(30));
    assert_eq!(p.case_, None); // 15 -> unknown
    assert!(!p.charging_left);
    assert!(p.charging_right);
    assert!(!p.charging_case);
  }

  #[test]
  fn battery_full_and_empty_edges() {
    // curr=0, anot=10
    let b = make(&[(5, 0b0010_0000), (6, 0xA0), (7, 0)]);
    let p = parse_apple_continuity(&b).unwrap();
    assert_eq!(p.left, Some(0));
    assert_eq!(p.right, Some(100));
  }
}
