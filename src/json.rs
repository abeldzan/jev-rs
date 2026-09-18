use serde::{Serialize, Serializer};
use serde_json::{Map, Value};

use crate::Error;

/// JSON content accepted by TypeSafe: a string, object, or array.
#[derive(Clone, Debug, PartialEq)]
pub struct JsonContent(Value);

impl JsonContent {
    /// Validates and wraps a JSON value.
    pub fn try_new(value: Value) -> Result<Self, Error> {
        match value {
            Value::String(_) | Value::Object(_) | Value::Array(_) => Ok(Self(value)),
            _ => Err(Error::validation(
                "JSON content must be a string, object, or array.",
            )),
        }
    }

    /// Returns the underlying JSON value.
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    /// Consumes this value and returns its JSON representation.
    pub fn into_value(self) -> Value {
        self.0
    }
}

impl Serialize for JsonContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl TryFrom<Value> for JsonContent {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<String> for JsonContent {
    fn from(value: String) -> Self {
        Self(Value::String(value))
    }
}

impl From<&str> for JsonContent {
    fn from(value: &str) -> Self {
        Self(Value::String(value.to_owned()))
    }
}

impl From<Map<String, Value>> for JsonContent {
    fn from(value: Map<String, Value>) -> Self {
        Self(Value::Object(value))
    }
}

impl From<Vec<Value>> for JsonContent {
    fn from(value: Vec<Value>) -> Self {
        Self(Value::Array(value))
    }
}

/// Conversion into a valid System One state value.
pub trait IntoState {
    /// Performs the conversion, rejecting invalid top-level JSON primitives.
    fn into_state(self) -> Result<JsonContent, Error>;
}

impl IntoState for JsonContent {
    fn into_state(self) -> Result<JsonContent, Error> {
        Ok(self)
    }
}

impl IntoState for Value {
    fn into_state(self) -> Result<JsonContent, Error> {
        JsonContent::try_new(self)
    }
}

impl IntoState for &Value {
    fn into_state(self) -> Result<JsonContent, Error> {
        JsonContent::try_new(self.clone())
    }
}

impl IntoState for String {
    fn into_state(self) -> Result<JsonContent, Error> {
        Ok(self.into())
    }
}

impl IntoState for &str {
    fn into_state(self) -> Result<JsonContent, Error> {
        Ok(self.into())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_only_supported_top_level_shapes() {
        for value in [json!("text"), json!({"nested": null}), json!([null, 1])] {
            assert!(JsonContent::try_new(value).is_ok());
        }
        for value in [json!(null), json!(true), json!(4)] {
            assert!(JsonContent::try_new(value).is_err());
        }
    }
}
