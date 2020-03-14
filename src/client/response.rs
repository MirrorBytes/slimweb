use std::time::Instant;

#[cfg(feature = "json")]
use serde_json::Value;

use crate::{
	stream::{ self, Stream, Compressed, Chunked },
	error::Error,
	body::Body,
	StatusInfo, GeneralInfo,
};


#[derive(Debug)]
pub struct ClientResponse {
	pub info: GeneralInfo,
	pub body: Body,
}

impl ClientResponse {
	/// Create a new ClientResponse using Stream (either http or https), and a deadline (if one is set).
	pub(crate) fn new(mut stream: Stream, deadline: &mut Option<(Instant, Instant)>) -> Result<ClientResponse, Error> {
		let mut info = stream::process_lines(&mut stream)?;
		let (compressed, chunked) = stream::check_encodings(&info.headers);

		let mut chunked = Chunked::new(&mut stream, None, chunked);
		let mut compressed = Compressed::new(&mut chunked, None, None, compressed);

		let mut content_length: Option<usize> = None;

		if let Some(length) = info.headers.get("Content-Length") {
			if let Ok(length) = length.parse::<usize>() {
				content_length = Some(length);
			}
		}

		let mut body = vec![];
		if let StatusInfo::Response(code, _) = info.status {
			if code <= 300 || code >= 308 {
				stream::read_to_end_until(&mut compressed, &mut body, content_length, deadline)?;
			}
		}

		// Remove hop-by-hop.
		info.headers.remove("Transfer-Encoding");

		Ok(ClientResponse {
			info,
			body: body.into(),
		})
	}

	/// Convert entire response into JSON.
	#[cfg(feature = "json")]
	pub fn json(&self) -> Value {
		serde_json::json!({
			"info": self.info.json(),
			"body": self.body.json()
		})
	}
}
