pub mod form_data;

use serde::Serializer;

pub fn serialize_uuid<S: Serializer>(uuid: &uuid::Uuid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&uuid.to_string())
}

pub fn serialize_datetime<S: Serializer, T: chrono::TimeZone>(datetime: &chrono::DateTime<T>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&datetime.to_rfc3339())
}