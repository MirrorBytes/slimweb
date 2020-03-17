use std::time::Instant;

use crate::{
	stream::{ self, Stream, Chunked, Compressed },
	error::Error,
	GeneralInfo,
};



#[derive(Debug)]
pub struct ServerRequest {
	pub info: GeneralInfo,
	pub body: Vec<u8>,
}

impl ServerRequest {
	pub(crate) fn new(stream: &mut Stream, info: GeneralInfo, deadline: &mut Option<(Instant, Instant)>) -> Result<ServerRequest, Error> {
		let headers = info.headers.clone();
		let (check_compressed, check_chunked) = stream::check_encodings(&headers);

		let mut content_length: Option<usize> = None;

		if !check_chunked {
			if let Some(length) = info.headers.get("Content-Length") {
				if let Ok(length) = length.parse::<usize>() {
					content_length = Some(length);
				}
			}
		}

		let mut body = vec![];

		if check_chunked || content_length.is_some() {
			let mut chunked = Chunked::new(stream, None, check_chunked);
			let mut compressed = Compressed::new(&mut chunked, None, None, check_compressed);

			stream::read_to_end_until(&mut compressed, &mut body, content_length, deadline)?;
		}

		Ok(ServerRequest {
			info,
			body,
		})
	}
}
