//! The few protobuf messages rzr exchanges with the THX service over ZeroMQ
//! (package `thx.sa`, from VSSrv's embedded descriptors; docs/HALLAZGOS.md).
//! No I/O. rzr never builds a `State` from scratch: it patches the one the
//! service returns, so fields it doesn't know (room, emitters...) go back
//! byte for byte.

/// Wire types used by these messages.
const VARINT: u8 = 0;
const FIXED64: u8 = 1;
const BYTES: u8 = 2;
const FIXED32: u8 = 5;

/// `thx.sa.State` fields rzr reads or changes.
pub mod state {
    pub const SEQUENCE_NUMBER: u32 = 1;
    pub const SPATIAL_ENABLED: u32 = 2;
    pub const HARDWARE_ID: u32 = 3;
    pub const OUTPUT_DEVICE: u32 = 4;
    pub const PRESET_NAME: u32 = 5;
    pub const DRC_ENABLED: u32 = 9;
    pub const DRC_LEVEL: u32 = 10;
    pub const BASS_BOOST: u32 = 13;
    pub const DIALOG_ENHANCEMENT: u32 = 14;
    pub const BASS_BOOST_ENABLED: u32 = 17;
    pub const DIALOG_ENHANCEMENT_ENABLED: u32 = 18;
}

/// One field of a message, as found on the wire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field<'a> {
    pub num: u32,
    pub value: Value<'a>,
    /// The whole field (key included), to copy it unchanged.
    pub raw: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value<'a> {
    Varint(u64),
    Fixed64([u8; 8]),
    Bytes(&'a [u8]),
    Fixed32([u8; 4]),
}

/// A change to one field of a `State`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Patch {
    Bool(u32, bool),
    Double(u32, f64),
}

impl Patch {
    fn num(self) -> u32 {
        match self {
            Patch::Bool(n, _) | Patch::Double(n, _) => n,
        }
    }
}

fn read_varint(b: &[u8], i: &mut usize) -> Result<u64, String> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*i).ok_or("protobuf cortado")?;
        *i += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("varint demasiado largo".into())
}

fn take<'a>(b: &'a [u8], i: &mut usize, n: usize) -> Result<&'a [u8], String> {
    let end = i.checked_add(n).filter(|&e| e <= b.len()).ok_or("protobuf cortado")?;
    let out = &b[*i..end];
    *i = end;
    Ok(out)
}

/// The top-level fields of a message, in wire order.
pub fn fields(b: &[u8]) -> Result<Vec<Field<'_>>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let key = read_varint(b, &mut i)?;
        let num = u32::try_from(key >> 3).map_err(|_| "número de campo inválido")?;
        let value = match (key & 7) as u8 {
            VARINT => Value::Varint(read_varint(b, &mut i)?),
            FIXED64 => Value::Fixed64(take(b, &mut i, 8)?.try_into().unwrap_or_default()),
            BYTES => {
                let n = usize::try_from(read_varint(b, &mut i)?).map_err(|_| "campo demasiado largo")?;
                Value::Bytes(take(b, &mut i, n)?)
            }
            FIXED32 => Value::Fixed32(take(b, &mut i, 4)?.try_into().unwrap_or_default()),
            w => return Err(format!("tipo de campo protobuf desconocido: {w}")),
        };
        out.push(Field { num, value, raw: &b[start..i] });
    }
    Ok(out)
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn put_key(out: &mut Vec<u8>, num: u32, wire: u8) {
    put_varint(out, u64::from(num) << 3 | u64::from(wire));
}

fn put_uint(out: &mut Vec<u8>, num: u32, v: u64) {
    put_key(out, num, VARINT);
    put_varint(out, v);
}

fn put_bytes(out: &mut Vec<u8>, num: u32, v: &[u8]) {
    put_key(out, num, BYTES);
    put_varint(out, v.len() as u64);
    out.extend_from_slice(v);
}

fn put_double(out: &mut Vec<u8>, num: u32, v: f64) {
    put_key(out, num, FIXED64);
    out.extend_from_slice(&v.to_le_bytes());
}

fn field<'a>(fields: &[Field<'a>], num: u32) -> Option<Value<'a>> {
    fields.iter().rev().find(|f| f.num == num).map(|f| f.value)
}

/// A `uint32`/`bool` field; absent means 0, as in proto3.
pub fn uint(msg: &[u8], num: u32) -> Result<u64, String> {
    Ok(match field(&fields(msg)?, num) {
        Some(Value::Varint(v)) => v,
        _ => 0,
    })
}

/// A `double` field; absent means 0.
pub fn double(msg: &[u8], num: u32) -> Result<f64, String> {
    Ok(match field(&fields(msg)?, num) {
        Some(Value::Fixed64(b)) => f64::from_le_bytes(b),
        _ => 0.0,
    })
}

/// A `string` field; absent means "".
pub fn string(msg: &[u8], num: u32) -> Result<String, String> {
    Ok(match field(&fields(msg)?, num) {
        Some(Value::Bytes(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => String::new(),
    })
}

/// The state has the value `patch` sets.
pub fn shows(state: &[u8], patch: Patch) -> Result<bool, String> {
    Ok(match patch {
        Patch::Bool(num, on) => (uint(state, num)? != 0) == on,
        Patch::Double(num, v) => (double(state, num)? - v).abs() < 1e-6,
    })
}

/// `thx.sa.THXMessage { Any msg = 1; }` holding `thx.sa.<name>`. Synapse
/// leaves the message's own `originator` field empty (it goes in the
/// `x-originator:` frame), and so does rzr.
fn thx_message(name: &str, value: &[u8]) -> Vec<u8> {
    let mut any = Vec::new();
    put_bytes(&mut any, 1, format!("type.googleapis.com/thx.sa.{name}").as_bytes());
    put_bytes(&mut any, 2, value);
    let mut out = Vec::new();
    put_bytes(&mut out, 1, &any);
    out
}

/// `thx.sa.Register { pid = 1 }`: the service answers with its current state.
pub fn register(pid: u32) -> Vec<u8> {
    let mut v = Vec::new();
    put_uint(&mut v, 1, pid.into());
    thx_message("Register", &v)
}

/// A new `thx.sa.State`: the service's `current` one with the next sequence
/// number and `patches` applied. The service only takes a state whose
/// sequence number is higher than its own.
pub fn next_state(current: &[u8], patches: &[Patch]) -> Result<Vec<u8>, String> {
    let fields = fields(current)?;
    let seq = match field(&fields, state::SEQUENCE_NUMBER) {
        Some(Value::Varint(v)) => v,
        _ => 0,
    };
    let replaced = |num| num == state::SEQUENCE_NUMBER || patches.iter().any(|p| p.num() == num);
    // Rebuilt in field-number order, like the service's own messages. Values
    // equal to the proto3 default are left out, as a protobuf library would.
    let mut parts: Vec<(u32, Vec<u8>)> =
        fields.iter().filter(|f| !replaced(f.num)).map(|f| (f.num, f.raw.to_vec())).collect();
    let mut seq_field = Vec::new();
    put_uint(&mut seq_field, state::SEQUENCE_NUMBER, seq + 1);
    parts.push((state::SEQUENCE_NUMBER, seq_field));
    for &p in patches {
        let mut raw = Vec::new();
        match p {
            Patch::Bool(num, true) => put_uint(&mut raw, num, 1),
            Patch::Double(num, v) if v != 0.0 => put_double(&mut raw, num, v),
            Patch::Bool(..) | Patch::Double(..) => {}
        }
        parts.push((p.num(), raw));
    }
    parts.sort_by_key(|(num, _)| *num);
    let body: Vec<u8> = parts.into_iter().flat_map(|(_, raw)| raw).collect();
    Ok(thx_message("State", &body))
}

/// `thx.sa.SetPreset { sequence_number = 1; PresetKey key = 3 }`, for the
/// preset `name` of the device described by the service's `current` state.
pub fn set_preset(current: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let seq = uint(current, state::SEQUENCE_NUMBER)?;
    // PresetKey { name = 1; spatial_enabled = 3; output_device = 4; hardware_id = 5 }
    let mut key = Vec::new();
    put_bytes(&mut key, 1, name.as_bytes());
    if uint(current, state::SPATIAL_ENABLED)? != 0 {
        put_uint(&mut key, 3, 1);
    }
    put_bytes(&mut key, 4, string(current, state::OUTPUT_DEVICE)?.as_bytes());
    put_bytes(&mut key, 5, string(current, state::HARDWARE_ID)?.as_bytes());
    let mut v = Vec::new();
    put_uint(&mut v, 1, seq + 1);
    put_bytes(&mut v, 3, &key);
    Ok(thx_message("SetPreset", &v))
}

/// The service's answer: `thx.sa.StateChangeResult { status = 1; msg = 2; State state = 3 }`.
#[derive(Debug, PartialEq)]
pub struct Reply {
    /// 0 = accepted.
    pub status: u64,
    pub msg: String,
    /// The service's state after the request.
    pub state: Vec<u8>,
}

pub fn parse_reply(payload: &[u8]) -> Result<Reply, String> {
    let bad = |what: &str| format!("respuesta del servicio de THX inesperada: {what}");
    let Some(Value::Bytes(any)) = field(&fields(payload)?, 1) else { return Err(bad("sin mensaje")) };
    let any = fields(any)?;
    let Some(Value::Bytes(url)) = field(&any, 1) else { return Err(bad("sin tipo")) };
    if url != b"type.googleapis.com/thx.sa.StateChangeResult" {
        return Err(bad(&String::from_utf8_lossy(url)));
    }
    let result = match field(&any, 2) {
        Some(Value::Bytes(b)) => b,
        _ => &[],
    };
    let state = match field(&fields(result)?, 3) {
        Some(Value::Bytes(b)) => b.to_vec(),
        _ => Vec::new(),
    };
    Ok(Reply { status: uint(result, 1)?, msg: string(result, 2)?, state })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `x-payload:` of the service's answer to a `Register`, captured on
    /// the user's PC (2026-09-26): THXMessage > Any > StateChangeResult
    /// { state = 3 }, with Spatial, Normalization and Bass Boost on.
    const CAPTURED_REPLY: &[u8] = include_bytes!("testdata/register-reply.bin");

    fn captured_state() -> Vec<u8> {
        parse_reply(CAPTURED_REPLY).unwrap().state
    }

    #[test]
    fn register_bytes() {
        // Same bytes as the Register the service answered (pid 123 here).
        let want = b"\x0a\x29\x0a\x23type.googleapis.com/thx.sa.Register\x12\x02\x08\x7b";
        assert_eq!(register(123), want.to_vec());
    }

    #[test]
    fn parses_the_service_reply() {
        let r = parse_reply(CAPTURED_REPLY).unwrap();
        assert_eq!((r.status, r.msg.as_str()), (0, ""));
        assert_eq!(uint(&r.state, state::SEQUENCE_NUMBER).unwrap(), 79);
        assert_eq!(uint(&r.state, state::SPATIAL_ENABLED).unwrap(), 1);
        assert_eq!(string(&r.state, state::PRESET_NAME).unwrap(), "Custom");
        assert_eq!(uint(&r.state, state::DRC_ENABLED).unwrap(), 1);
        assert_eq!(double(&r.state, state::DRC_LEVEL).unwrap(), 100.0);
        assert_eq!(double(&r.state, state::BASS_BOOST).unwrap(), 100.0);
        assert_eq!(double(&r.state, state::DIALOG_ENHANCEMENT).unwrap(), 100.0);
        assert_eq!(uint(&r.state, 17).unwrap(), 1);
        assert_eq!(uint(&r.state, 18).unwrap(), 0, "absent bool reads as false");
    }

    #[test]
    fn rejects_other_answers() {
        assert!(parse_reply(&register(1)).is_err());
        assert!(parse_reply(b"\x0a\x05\x0a").is_err());
        assert!(parse_reply(b"").is_err());
    }

    /// The State inside a THXMessage built by rzr.
    fn inner_state(msg: &[u8]) -> Vec<u8> {
        let Some(Value::Bytes(any)) = field(&fields(msg).unwrap(), 1) else { panic!() };
        let any = fields(any).unwrap();
        assert_eq!(field(&any, 1), Some(Value::Bytes(b"type.googleapis.com/thx.sa.State")));
        let Some(Value::Bytes(state)) = field(&any, 2) else { panic!() };
        state.to_vec()
    }

    #[test]
    fn next_state_patches_and_keeps_the_rest() {
        let current = captured_state();
        let patches = [
            Patch::Double(state::BASS_BOOST, 35.0),
            Patch::Bool(state::DRC_ENABLED, false),
            Patch::Double(state::DIALOG_ENHANCEMENT, 0.0),
            Patch::Bool(state::DIALOG_ENHANCEMENT_ENABLED, true),
        ];
        let new = inner_state(&next_state(&current, &patches).unwrap());
        assert_eq!(uint(&new, state::SEQUENCE_NUMBER).unwrap(), 80);
        for p in patches {
            assert!(shows(&new, p).unwrap(), "{p:?}");
            assert!(!shows(&current, p).unwrap(), "{p:?} was already there");
        }
        // Defaults are left out, like a protobuf library would do.
        let nums: Vec<u32> = fields(&new).unwrap().iter().map(|f| f.num).collect();
        assert_eq!(nums, [1, 2, 3, 4, 5, 6, 8, 10, 11, 12, 13, 15, 17, 18]);
        // Fields rzr doesn't touch (curve, emitters, room, upmix...) travel unchanged.
        let old = fields(&current).unwrap();
        for f in fields(&new).unwrap().iter().filter(|f| ![1, 13, 18].contains(&f.num)) {
            assert_eq!(Some(f.raw), old.iter().find(|o| o.num == f.num).map(|o| o.raw), "field {}", f.num);
        }
        // With nothing to patch, only the sequence number changes.
        let same = inner_state(&next_state(&current, &[]).unwrap());
        assert_eq!(same.len(), current.len());
        assert_eq!(fields(&same).unwrap()[1..], fields(&current).unwrap()[1..]);
    }

    #[test]
    fn set_preset_names_this_device() {
        let current = captured_state();
        let msg = set_preset(&current, "Music Mode").unwrap();
        let Some(Value::Bytes(any)) = field(&fields(&msg).unwrap(), 1) else { panic!() };
        let any = fields(any).unwrap();
        assert_eq!(field(&any, 1), Some(Value::Bytes(b"type.googleapis.com/thx.sa.SetPreset")));
        let Some(Value::Bytes(v)) = field(&any, 2) else { panic!() };
        assert_eq!(uint(v, 1).unwrap(), 80);
        let Some(Value::Bytes(key)) = field(&fields(v).unwrap(), 3) else { panic!() };
        assert_eq!(string(key, 1).unwrap(), "Music Mode");
        assert_eq!(uint(key, 3).unwrap(), 1);
        assert_eq!(string(key, 4).unwrap(), "Headphones");
        assert_eq!(string(key, 5).unwrap(), r"usb\1532\0555");
    }

    #[test]
    fn varints_round_trip_and_bad_input_fails() {
        for v in [0u64, 1, 127, 128, 300, 16_384, u64::from(u32::MAX), u64::MAX] {
            let mut b = Vec::new();
            put_varint(&mut b, v);
            assert_eq!(read_varint(&b, &mut 0).unwrap(), v);
        }
        assert!(fields(b"\x0a\x05ab").is_err(), "length past the end");
        assert!(fields(b"\x08\x80").is_err(), "varint cut");
        assert!(fields(b"\x0b").is_err(), "group wire type");
    }
}
