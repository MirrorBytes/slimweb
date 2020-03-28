//! Slim HTTP 1.1 client/server library.
//!
//! I felt compelled (or inspired if you will) to write this library after reading this article:
//! https://medium.com/@shnatsel/smoke-testing-rust-http-clients-b8f2ee5db4e6
//!
//! More on the controversial side of the Rust community, it seemed quite interesting how such
//! eloquent libraries could be riddled down to such minor details that could cause major problems.
//! So, I'm throwing another into the mix that will probably hit that same point.
//!
//! No async functionality.
//! Decisively using deadlines for DoS prevention (didn't want to deal with leaky thread racing).
//! Using Rustls for SSL/TLS encryption.
//! Using flate2 for compression/decompression (GZip only).

#![deny(clippy::all, missing_docs)]
#![forbid(unsafe_code)]

use std::collections::HashMap;

#[cfg(feature = "server")]
#[macro_use] extern crate log;

#[cfg(feature = "json")]
use serde_json::Value;

#[macro_use] mod macros;
mod error;
mod body;
#[cfg(feature = "multipart")] mod multipart;
mod stream;
#[cfg(feature = "client")] mod client;
#[cfg(feature = "server")] mod server;

pub use error::*;
#[cfg(feature = "multipart")] pub use multipart::*;
#[cfg(feature = "client")] pub use client::*;
#[cfg(feature = "server")] pub use server::*;



/// General status info.
#[derive(Debug, Clone, PartialEq)]
pub enum StatusInfo {
	/// General response status information (code, reason).
	Response(i32, String), // server response
	/// General request status information (method, resource).
	Request(String, String), // client request
}

/// General response info.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneralInfo {
	/// HTTP Status Line.
	pub status: StatusInfo,
	/// Response headers.
	pub headers: HashMap<String, String>,
}

#[cfg(feature = "json")]
impl GeneralInfo {
	/// Convert response information into JSON.
	pub fn json(&self) -> Value {
		match &self.status {
			StatusInfo::Response(code, reason) => {
				serde_json::json!({
					"status": {
						"code": code,
						"reason": reason
					},
					"headers": self.headers
				})
			},
			StatusInfo::Request(method, resource) => {
				serde_json::json!({
					"status": {
						"method": method,
						"resource": resource
					},
					"headers": self.headers
				})
			},
		}
	}
}
