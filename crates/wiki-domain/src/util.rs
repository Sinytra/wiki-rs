use tracing::error;
use std::fmt;
use serde::{de, Deserialize, Deserializer};
use serde::de::{SeqAccess, Visitor};

pub const BUILTIN_PROJECT_ID: &str = "minecraft";

pub trait LogErr<T, E> {
    fn log_err(self, msg: &str);

    fn inspect_err_log(self, msg: &str) -> Result<T, E>;

    fn map_err_log<U, F>(self, msg: &str, f: F) -> Result<T, U>
    where
        F: FnOnce(E) -> U;
}

impl<T, E: std::fmt::Display> LogErr<T, E> for Result<T, E> {
    fn log_err(self, msg: &str) {
        if let Err(e) = self {
            error!(error = %e, "{msg}");
        }
    }

    fn inspect_err_log(self, msg: &str) -> Result<T, E> {
        if let Err(e) = &self {
            error!(error = %e, "{msg}");
        }
        self
    }

    #[inline]
    fn map_err_log<U, F>(self, msg: &str, f: F) -> Result<T, U>
    where
        F: FnOnce(E) -> U,
    {
        self.map_err(|e| {
            error!(error = %e, "{msg}");
            f(e)
        })
    }
}

pub fn string_or_seq<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrSeqVisitor;

    impl<'de> Visitor<'de> for StringOrSeqVisitor {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a string or array of strings")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            Ok(vec![v.to_owned()])
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Vec<String>, E> {
            Ok(vec![v])
        }

        fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Vec<String>, A::Error> {
            Vec::<String>::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(StringOrSeqVisitor)
}