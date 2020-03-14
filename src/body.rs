#[cfg(feature = "json")]
use serde_json::Value;



/// Possible bodies used by both requests and responses.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
	/// Plain text
	Text(String),
	/// Converted bytes
	Bytes(Vec<u8>),
	/// JSON
	#[cfg(feature = "json")]
	Json(Value),
}

impl From<&str> for Body {
	fn from(body: &str) -> Body {
		Body::Text(body.to_owned())
	}
}

impl From<Vec<u8>> for Body {
	fn from(body: Vec<u8>) -> Body {
		Body::Bytes(body)
	}
}

impl From<&Vec<u8>> for Body {
	fn from(body: &Vec<u8>) -> Body {
		Body::Bytes(body.to_owned())
	}
}

#[cfg(feature = "json")]
impl From<Value> for Body {
	fn from(body: Value) -> Body {
		Body::Json(body)
	}
}

impl Into<Vec<u8>> for Body {
	fn into(self) -> Vec<u8> {
		match self {
			Body::Text(text) => text.as_bytes().to_vec(),
			Body::Bytes(bytes) => bytes,
			#[cfg(feature = "json")]
			Body::Json(value) => serde_json::to_vec(&value).expect("Bad JSON value"),
		}
	}
}

impl Into<Vec<u8>> for &Body {
	fn into(self) -> Vec<u8> {
		match self {
			Body::Text(text) => text.as_bytes().to_vec(),
			Body::Bytes(bytes) => bytes.to_owned(),
			#[cfg(feature = "json")]
			Body::Json(value) => serde_json::to_vec(&value).expect("Bad JSON value"),
		}
	}
}

impl Into<String> for &Body {
	fn into(self) -> String {
		match self {
			Body::Text(text) => text.to_string(),
			Body::Bytes(bytes) => String::from_utf8_lossy(&bytes).to_string(),
			#[cfg(feature = "json")]
			Body::Json(value) => value.to_string(),
		}
	}
}

#[cfg(feature = "json")]
impl Into<Value> for &Body {
	fn into(self) -> Value {
		match self {
			Body::Text(text) => serde_json::from_str(text).unwrap(),
			Body::Bytes(bytes) => serde_json::from_slice(&bytes).unwrap(),
			#[cfg(feature = "json")]
			Body::Json(value) => value.to_owned(),
		}
	}
}

impl Body {
	/// Convert body into text.
	pub fn text(&self) -> String {
		self.into()
	}

	/// Convert body into JSON.
	#[cfg(feature = "json")]
	pub fn json(&self) -> Value {
		self.into()
	}
}
