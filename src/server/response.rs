use std::{
	io::Write,
	collections::HashMap,
};

use crate::{
	error::Error,
	body::Body,
	StatusInfo, GeneralInfo,
};



impl Into<Vec<u8>> for StatusInfo {
	fn into(self) -> Vec<u8> {
		match self {
			StatusInfo::Response(code, reason) => {
				let mut head = vec![];

				writeln!(head, "HTTP/1.1 {} {}\r", code, reason).unwrap();

				head
			},
			_ => vec![],
		}
	}
}

impl Into<Vec<u8>> for GeneralInfo {
	fn into(self) -> Vec<u8> {
		let mut head = vec![];

		head.extend::<Vec<u8>>(self.status.into());

		for (k, v) in &self.headers {
			writeln!(head, "{}: {}\r", k, v).unwrap();
		}

		head
	}
}



/// Response built by user to return to requester.
#[derive(Debug)]
pub struct ServerResponse {
	/// Generic response information.
	pub info: GeneralInfo,
	/// Response body.
	pub body: Option<Body>,
	/// Compression level (0-9).
	pub compression_level: Option<u32>,
	/// Chunk size.
	pub chunk_size: Option<usize>,
}

impl Into<Vec<u8>> for ServerResponse {
	fn into(self) -> Vec<u8> {
		let mut head = vec![];

		head.extend::<Vec<u8>>(self.info.into());

		writeln!(head, "\r").unwrap();

		if let Some(body) = self.body {
			head.extend::<Vec<u8>>(body.into());
		}

		head
	}
}

impl ServerResponse {
	/// Creates new response to send to requester.
	pub fn new(status: i32) -> Result<ServerResponse, Error> {
		let reason = match status {
			100 => "Continue",
			101 => "Switching Protocols",
			103 => "Early Hints",
			200 => "OK",
			201 => "Created",
			202 => "Accepted",
			203 => "Non-Authoritative Information",
			204 => "No Content",
			205 => "Reset Content",
			206 => "Partial Content",
			300 => "Multiple Choices",
			301 => "Moved Permanently",
			302 => "Found",
			303 => "See Other",
			304 => "Not Modified",
			305 => "Use Proxy",
			306 => "Switch Proxy",
			307 => "Temporary Redirect",
			308 => "Permanent Redirect",
			400 => "Bad Request",
			401 => "Unauthorized",
			402 => "Payment Required",
			403 => "Forbidden",
			404 => "Not Found",
			405 => "Method Not Allowed",
			406 => "Not Acceptable",
			407 => "Proxy Authentication Required",
			408 => "Request Timeout",
			409 => "Conflict",
			410 => "Gone",
			411 => "Length Required",
			412 => "Precondition Failed",
			413 => "Payload Too Large",
			414 => "URI Too Long",
			415 => "Unsupported Media Type",
			416 => "Range Not Satisfiable",
			417 => "Expectation Failed",
			418 => "I'm a teapot",
			421 => "Misdirected Request",
			425 => "Too Early",
			426 => "Upgrade Required",
			428 => "Precondition Required",
			429 => "Too Many Requests",
			431 => "Request Header Fields Too Large",
			451 => "Unavailable For Legal Reasons",
			500 => "Internal Server Error",
			501 => "Not Implemented",
			502 => "Bad Gateway",
			503 => "Service Unavailable",
			504 => "Gateway Timeout",
			505 => "HTTP Version Not Supported",
			506 => "Variant Also Negotiates",
			510 => "Not Extended",
			511 => "Network Authentication Required",
			_ => return Err(Error::HTTPStatusCodeNotRecognized),
		};

		Ok(ServerResponse {
			info: GeneralInfo {
				status: StatusInfo::Response(status, reason.into()),
				headers: HashMap::new(),
			},
			body: None,
			compression_level: None,
			chunk_size: None,
		})
	}

	/// Sets/replaces individual header for response.
	pub fn set_header<S: Into<String>>(mut self, key: S, value: S) -> ServerResponse {
		self.info.headers.insert(key.into(), value.into());

		self
	}

	/// Sets request body.
	pub fn set_body<B: Into<Body>>(mut self, body: B) -> ServerResponse {
		// Convert supplied body to Body.
		let body = body.into();

		// Set Content-Type header based on Body type.
		match body {
			#[cfg(feature = "json")]
			Body::Json(_) => {
				self.info.headers.insert("Content-Type".into(), "application/json;charset=UTF-8".into());
			},
			_ => {
				self.info.headers.insert("Content-Type".into(), "text/plain;".into());
			},
		}

		// Set request body and return request.
		self.body = Some(body);

		self
	}

	/// Sets response chunk size.
	pub fn set_chunk_size(mut self, chunk_size: usize) -> ServerResponse {
		self.chunk_size = Some(chunk_size);

		self
	}

	/// Sets response compression level.
	#[cfg(feature = "compress")]
	pub fn set_compression_level(mut self, level: u32) -> ServerResponse {
		self.compression_level = Some(level);

		self
	}
}
