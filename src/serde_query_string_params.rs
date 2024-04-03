use std::fmt::Display;

use serde::{de, Deserialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(PartialEq, Debug, Deserialize)]
    struct OneStringField {
        field_1: String,
    }

    #[derive(PartialEq, Debug, Deserialize)]
    struct TwoStringFields {
        field_1: String,
        field_2: String,
    }

    #[derive(PartialEq, Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TwoStringFieldsStrict {
        field_1: String,
        field_2: String,
    }

    #[test]
    fn one_string_field() {
        let res: OneStringField = from_str("field_1=abcde").unwrap();
        assert_eq!(
            res,
            OneStringField {
                field_1: "abcde".into()
            }
        )
    }

    #[test]
    fn two_string_fields() {
        let res: TwoStringFields = from_str("field_1=abcde&field_2=anotherField").unwrap();
        assert_eq!(
            res,
            TwoStringFields {
                field_1: "abcde".into(),
                field_2: "anotherField".into()
            }
        )
    }

    #[test]
    fn extra_field() {
        let res: TwoStringFields =
            from_str("field_1=firstField&field_2=secondField&field_3=extraField").unwrap();
        assert_eq!(
            res,
            TwoStringFields {
                field_1: "firstField".into(),
                field_2: "secondField".into()
            }
        );

        let res: TwoStringFields =
            from_str("field_1=firstField&field_3=extraField&field_2=secondField").unwrap();
        assert_eq!(
            res,
            TwoStringFields {
                field_1: "firstField".into(),
                field_2: "secondField".into()
            }
        );
    }

    #[test]
    fn fails_on_extra_field() {
        let res: Result<TwoStringFieldsStrict, _> =
            from_str("field_1=firstField&field_2=secondField&field_3=extraField");
        match res {
            Err(Error::CustomMessage(msg)) => {
                assert!(msg.starts_with("unknown field `field_3`"))
            }
            Err(_) => assert!(false, "Incorrect error variant, expected CustomMessage"),
            Ok(_) => assert!(false, "Didn't fail"),
        }

        let res: Result<TwoStringFieldsStrict, _> =
            from_str("field_1=firstField&field_3=extraField&field_2=secondField");
        match res {
            Err(Error::CustomMessage(msg)) => {
                assert!(msg.starts_with("unknown field `field_3`"))
            }
            Err(_) => assert!(false, "Incorrect error variant, expected CustomMessage"),
            Ok(_) => assert!(false, "Didn't fail"),
        }
    }

    #[test]
    fn decodes_special_symbols() {
        let res: OneStringField = from_str("field_1=%D0%9F%D1%80%D0%B8%D0%B2%D0%B5%D1%82%21").unwrap();
        assert_eq!(res.field_1, "Привет!");
    }

    #[test]
    fn decodes_spaces_pluses() {
        let res: OneStringField = from_str("field_1=How+are+you").unwrap();
        assert_eq!(res.field_1, "How are you");
    }

    #[test]
    fn decodes_spaces_percent() {
        let res: OneStringField = from_str("field_1=How%20are%20you").unwrap();
        assert_eq!(res.field_1, "How are you");
    }

    #[test]
    fn decodes_pluses() {
        let res: OneStringField = from_str("field_1=Me%2Byou").unwrap();
        assert_eq!(res.field_1, "Me+you");
    }

    #[test]
    fn decodes_emojis() {
        let res: OneStringField = from_str("field_1=Lol%F0%9F%98%82%21").unwrap();
        assert_eq!(res.field_1, "Lol😂!");
    }
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T, Error>
where
    T: Deserialize<'a>,
{
    let deserializer = Deserializer::new(s);
    let t = T::deserialize(deserializer)?;
    Ok(t)
}

macro_rules! de_unsupported {
    ($func_name:ident) => {
        fn $func_name<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'de>,
        {
            Err(Error::Unsupported(stringify!($func_name)))
        }
    };
    ($func_name:ident, $($arg:ident: $arg_type:ty),*) => {
        fn $func_name<V>(self, $($arg: $arg_type,)* _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'de>,
        {
            Err(Error::Unsupported(stringify!($func_name)))
        }
    };
}

#[derive(Debug, PartialEq)]
pub enum Error {
    CustomMessage(String),
    Unsupported(&'static str),
    CantParseKey(String),
    CantParseValue(String),
}

impl serde::de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::CustomMessage(msg.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::CustomMessage(msg) => formatter.write_str(msg),
            Error::Unsupported(s) => formatter.write_str(&format!("unsupported operation: {s}")),
            Error::CantParseKey(s) => formatter.write_str(&format!("Can't parse key from: {s}")),
            Error::CantParseValue(s) => {
                formatter.write_str(&format!("Can't parse value from: {s}"))
            }
        }
    }
}

impl std::error::Error for Error {}

struct Deserializer<'de> {
    str: &'de str,
}

impl<'de> Deserializer<'de> {
    fn new(s: &'de str) -> Self {
        Deserializer { str: s }
    }
}
impl<'de> de::Deserializer<'de> for Deserializer<'de> {
    type Error = Error;

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> {
        let map = KeyValuePairs::new(self.str);
        visitor.visit_map(map)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> {
        self.deserialize_map(visitor)
    }

    de_unsupported!(deserialize_any);
    de_unsupported!(deserialize_bool);
    de_unsupported!(deserialize_i8);
    de_unsupported!(deserialize_i16);
    de_unsupported!(deserialize_i32);
    de_unsupported!(deserialize_i64);
    de_unsupported!(deserialize_u8);
    de_unsupported!(deserialize_u16);
    de_unsupported!(deserialize_u32);
    de_unsupported!(deserialize_u64);
    de_unsupported!(deserialize_f32);
    de_unsupported!(deserialize_f64);
    de_unsupported!(deserialize_char);
    de_unsupported!(deserialize_bytes);
    de_unsupported!(deserialize_byte_buf);
    de_unsupported!(deserialize_option);
    de_unsupported!(deserialize_unit);
    de_unsupported!(deserialize_seq);
    de_unsupported!(deserialize_str);
    de_unsupported!(deserialize_string);
    de_unsupported!(deserialize_identifier);
    de_unsupported!(deserialize_ignored_any);
    de_unsupported!(deserialize_tuple, _len: usize);
    de_unsupported!(deserialize_unit_struct, _name: &'static str);
    de_unsupported!(deserialize_newtype_struct, _name: &'static str);
    de_unsupported!(deserialize_tuple_struct, _name: &'static str, _len: usize);
    de_unsupported!(deserialize_enum, _name: &'static str, _variants: &'static [&'static str]);
}

struct KeyValuePairs<'de> {
    str: &'de str,
}

impl<'de> KeyValuePairs<'de> {
    fn new(str: &'de str) -> Self {
        KeyValuePairs { str }
    }
}

impl<'de> serde::de::MapAccess<'de> for KeyValuePairs<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        if self.str.is_empty() {
            return Ok(None);
        };

        match self.str.split_once('=') {
            Some((key, rest)) => {
                self.str = rest;
                seed.deserialize(StringValue(key)).map(Some)
            }
            None => Err(Error::CantParseKey(self.str.into())),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        if self.str.is_empty() {
            return Err(Error::CantParseValue(self.str.into()));
        };

        match self.str.split_once('&') {
            Some((value, rest)) => {
                self.str = rest;
                seed.deserialize(StringValue(value))
            }
            None => {
                let value = self.str;
                self.str = "";
                seed.deserialize(StringValue(value))
            }
        }
    }
}

struct StringValue<'de>(&'de str);

impl<'de> de::Deserializer<'de> for StringValue<'de> {
    type Error = Error;

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(decode(self.0))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(decode(self.0))
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_borrowed_str("")
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> {
        visitor.visit_some(self)
    }

    de_unsupported!(deserialize_str);
    de_unsupported!(deserialize_any);
    de_unsupported!(deserialize_bool);
    de_unsupported!(deserialize_i8);
    de_unsupported!(deserialize_i16);
    de_unsupported!(deserialize_i32);
    de_unsupported!(deserialize_i64);
    de_unsupported!(deserialize_u8);
    de_unsupported!(deserialize_u16);
    de_unsupported!(deserialize_u32);
    de_unsupported!(deserialize_u64);
    de_unsupported!(deserialize_f32);
    de_unsupported!(deserialize_f64);
    de_unsupported!(deserialize_char);
    de_unsupported!(deserialize_bytes);
    de_unsupported!(deserialize_byte_buf);
    de_unsupported!(deserialize_unit);
    de_unsupported!(deserialize_seq);
    de_unsupported!(deserialize_map);
    de_unsupported!(deserialize_tuple, _len: usize);
    de_unsupported!(deserialize_unit_struct, _name: &'static str);
    de_unsupported!(deserialize_newtype_struct, _name: &'static str);
    de_unsupported!(deserialize_tuple_struct, _name: &'static str, _len: usize);
    de_unsupported!(deserialize_enum, _name: &'static str, _variants: &'static [&'static str]);
    de_unsupported!(deserialize_struct, _name: &'static str, _fields: &'static [&'static str]);
}

fn decode<'a>(text: &'a str) -> String {
    let mut res = String::new();
    url_escape::decode_to_string(text.replace("+", " "), &mut res);
    res
}