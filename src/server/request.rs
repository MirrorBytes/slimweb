use std::time::Instant;

use crate::{
	multipart,
	stream::{ self, Stream, Chunked, Compressed },
	error::Error,
	body::Body,
	GeneralInfo,
};



#[derive(Debug)]
pub struct ServerRequest {
	pub info: GeneralInfo,
	pub body: Body,
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

		let mut body_bytes = vec![];

		if check_chunked || content_length.is_some() {
			let mut chunked = Chunked::new(stream, None, check_chunked);
			let mut compressed = Compressed::new(&mut chunked, None, None, check_compressed);

			stream::read_to_end_until(&mut compressed, &mut body_bytes, content_length, deadline)?;
		}

		let mut body: Body = body_bytes.clone().into();

		if let Some(content_type) = info.headers.get("Content-Type") {
			let type_split: Vec<&str> = content_type.split(';').collect();

			if type_split[0] == "multipart/form-data" {
				body = multipart::from_bytes(type_split[1], body_bytes)?.into();
			}
		}

		Ok(ServerRequest {
			info,
			body,
		})
	}
}
