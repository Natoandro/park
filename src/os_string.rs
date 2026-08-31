use std::ffi::OsString;
use std::fmt::Write as _;
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(crate) fn serialize<S>(value: &OsString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&encode_hex(value.as_bytes()))
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<OsString, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    decode_hex(&value)
        .map(OsString::from_vec)
        .map_err(serde::de::Error::custom)
}

pub(crate) mod vec {
    use super::*;

    pub(crate) fn serialize<S>(values: &[OsString], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = values
            .iter()
            .map(|value| encode_hex(value.as_bytes()))
            .collect::<Vec<_>>();
        encoded.serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<OsString>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<String>::deserialize(deserializer)?;
        values
            .into_iter()
            .map(|value| {
                decode_hex(&value)
                    .map(OsString::from_vec)
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("encoded OS string has odd length".to_owned());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| "invalid hexadecimal OS string".to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_arbitrary_bytes() {
        let encoded = encode_hex(b"park-\x80-\xff");
        assert_eq!(encoded, "7061726b2d802dff");
        assert_eq!(decode_hex(&encoded), Ok(b"park-\x80-\xff".to_vec()));
    }

    #[test]
    fn rejects_invalid_hex() {
        assert_eq!(
            decode_hex("abc"),
            Err("encoded OS string has odd length".to_owned())
        );
        assert_eq!(
            decode_hex("gg"),
            Err("invalid hexadecimal OS string".to_owned())
        );
    }
}
