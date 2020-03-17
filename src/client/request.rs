use std::{
	collections::HashMap,
	time::{ Instant, Duration },
	io::{ BufReader, Write },
};
#[cfg(feature = "tls")]
use std::sync::Arc;

#[cfg(feature = "tls")]
use rustls::{ ClientConfig, ClientSession, StreamOwned };
#[cfg(feature = "tls")]
use webpki::DNSNameRef;
#[cfg(feature = "tls")]
use webpki_roots::TLS_SERVER_ROOTS;

use crate::{
	stream::{ self, Stream, Compressed, Chunked },
	error::Error,
	body::Body,
	StatusInfo,
};
use super::response::ClientResponse;



#[cfg(feature = "tls")]
lazy_static::lazy_static! {
	static ref TLS_CONFIG: Arc<ClientConfig> = {
		let mut config = ClientConfig::new();

		config.root_store
			.add_server_trust_anchors(&TLS_SERVER_ROOTS);

		Arc::new(config)
	};
}

#[derive(Debug, PartialEq)]
struct Url {
	https: bool,
	// username, password
	credentials: Option<(String, String)>,
	host: String,
	resource: String,
}

/// Client request (wraps standard HTTP request).
#[derive(Debug)]
pub struct ClientRequest {
	method: String,
	url: Url,
	headers: HashMap<String, String>,
	body: Option<Body>,

	max_redirects: usize,
	redirects: Vec<(bool, String, String)>,

	compression: bool,
	check_target_encodings: bool,
	compression_level: Option<u32>,
	chunk_size: Option<usize>,

	// deadline, reset
	deadline: Option<(Instant, Instant)>,
}

impl ClientRequest {
	/// Create a new Request with desired method and URL.
	pub(crate) fn new<S: Into<String>>(method: S, url: S) -> Result<ClientRequest, Error> {
		let url = parse_url(url.into())?;

		Ok(ClientRequest {
			method: method.into(),
			url,
			headers: HashMap::new(),
			body: None,

			max_redirects: 5,
			redirects: vec![],

			compression: false,
			check_target_encodings: false,
			compression_level: None,
			chunk_size: None,

			deadline: None,
		})
	}

	/// Sets/replaces individual header for request.
	pub fn set_header<S: Into<String>>(mut self, key: S, value: S) -> ClientRequest {
		self.headers.insert(key.into(), value.into());

		self
	}

	/// Sets request body.
	pub fn set_body<B: Into<Body>>(mut self, body: B) -> ClientRequest {
		// Convert supplied body to Body.
		let body = body.into();

		// Set Content-Type header based on Body type.
		match body {
			#[cfg(feature = "json")]
			Body::Json(_) => {
				self.headers.insert("Content-Type".into(), "application/json;charset=UTF-8".into());
			},
			_ => {
				self.headers.insert("Content-Type".into(), "text/plain;".into());
			},
		}

		// Set request body and return request.
		self.body = Some(body);

		self
	}

	/// Sets deadline for request.
	pub fn set_deadline(mut self, time: u64) -> ClientRequest {
		self.deadline = Some((Instant::now() + Duration::from_secs(time), Instant::now()));

		self
	}

	/// Sets max redirects.
	pub fn set_max_redirects(mut self, redirects: usize) -> ClientRequest {
		self.max_redirects = redirects;

		self
	}

	/// Enable compression.
	///
	/// Enables for both request (if [`request.enable_compression()`](struct.ClientRequest.html#method.enable_compression) is called) and response.
	#[cfg(feature = "compress")]
	pub fn enable_compression(mut self) -> ClientRequest {
		self.compression = true;

		self
	}

	/// Checks target's encodings.
	/// Recommended for larger requests.
	///
	/// This is useless if you don't enabled compression via [`request.enable_compression()`](struct.ClientRequest.html#method.enable_compression)
	/// OR set a chunk size via [`request.set_chunk_size()`](struct.ClientRequest.html#method.set_chunk_size)
	pub fn check_target_encodings(mut self) -> ClientRequest {
		self.check_target_encodings = true;

		self
	}

	/// Sets request compression level.
	///
	/// Only used if [`request.check_target_encodings()`](struct.ClientRequest.html#method.check_target_encodings) AND [`request.enable_compression()`](struct.ClientRequest.html#method.enable_compression) is called.
	#[cfg(feature = "compress")]
	pub fn set_compression_level(mut self, level: u32) -> ClientRequest {
		if self.compression && self.check_target_encodings {
			self.compression_level = Some(level);
		}

		self
	}

	/// Sets request chunk size.
	pub fn set_chunk_size(mut self, chunk_size: usize) -> ClientRequest {
		if self.check_target_encodings {
			self.chunk_size = Some(chunk_size);
		}

		self
	}

	/// Sends request.
	pub fn send(mut self) -> Result<ClientResponse, Error> {
		self.url.host = ensure_ascii(self.url.host)?;

		let tcp = stream::connect(&self.url.host, self.deadline)?;

		let mut req_stream;
		if self.url.https {
			#[cfg(not(feature = "tls"))]
			{ return Err(Error::TLSNotEnabled); }

			#[cfg(feature = "tls")]
			{
				let mut name = self.url.host.clone();

				// Ditch the port. Safe due to added port above.
				name = name.split(':').next().unwrap().to_string();

				// Safe unwrap due to ASCII check above.
				let name = DNSNameRef::try_from_ascii_str(&name).unwrap();

				req_stream = Stream::HttpsClient(BufReader::new(Box::new(StreamOwned::new(ClientSession::new(&TLS_CONFIG, name), tcp))));
			}
		} else {
			req_stream = Stream::Http(BufReader::new(tcp));
		}

		// Check if compression is to be used for body.
		if self.compression && self.check_target_encodings {
			let mut check_req = ClientRequest::new("OPTIONS", &self.url.host)?;

			if self.deadline.is_some() {
				check_req.deadline = self.deadline;
			}

			let check_resp = check_req.send()?;

			if let Some(accept) = check_resp.info.headers.get("Accept-Encoding") {
				self.compression = accept
					.split(',')
					.map(|s| s.trim())
					.any(|s| s.eq_ignore_ascii_case("gzip"));
			}
		}

		let req = gen_head(&self)?;

		// Write head to stream. Clear req.
		if let Some((line, _)) = &mut self.deadline {
			let now = Instant::now();

			req_stream
				.get_ref()
				.set_write_timeout(Some(*line - now))?;

			self.deadline = Some((*line, Instant::now()));
		}

		req_stream.write_all(&req)?;
		req_stream.flush()?;

		if let Some(body) = &self.body {
			let mut chunked = Chunked::new(
				&mut req_stream,
				self.chunk_size,
				self.chunk_size.is_some()
			);

			let body: Vec<u8> = body.into();

			let mut compressed = if self.compression && self.compression_level.is_some() {
				Compressed::new(
					&mut chunked,
					self.compression_level,
					Some(&body),
					self.compression
				)
			} else { // Just in case compression level not set OR compression isn't being used.
				Compressed::new(
					&mut chunked,
					Some(0),
					Some(&body),
					self.compression
				)
			};

			// Write body to stream.
			stream::write_all_until(&mut compressed, &body, &mut self.deadline)?;
		}

		// Get response from Stream.
		let resp = ClientResponse::new(req_stream, &mut self.deadline)?;

		// Grab status code from response.
		let mut status_code = 0;
		if let StatusInfo::Response(code, _) = resp.info.status {
			status_code = code;
		}

		// Handle redirects.
		if status_code >= 300 && status_code <= 308 {
			if self.redirects.len() == self.max_redirects {
				Err(Error::MaxRedirectsHit)
			} else if let Some(location) = resp.info.headers.get("Location") {
				self.redirects.push((self.url.https, self.url.host.clone(), self.url.resource));

				let method = match self.method.as_str() {
					"GET" | "HEAD" => self.method,
					_ => "GET".into(),
				};
				let new_host = self.url.host + location.trim();
				let url = parse_url(new_host)?;

				// Reset necessary internals.
				self.method = method;
				self.url.https = url.https;
				self.url.host = url.host;
				self.url.resource = url.resource;

				self.send()
			} else {
				Err(Error::NoLocationHeader)
			}
		} else {
			Ok(resp)
		}
	}
}



// -----------------------------------------------------------------------------------------------------------
// Helper functions

/// Parses URL passed by creation of Request.
fn parse_url(url: String) -> Result<Url, Error> {
	// Check if it's a secured connection.
	let https = url.starts_with("https://");

	// Reset slice of url after protocol (if one exists, otherwise, assume http)
	let mut url = if https {
		&url[8..]
	} else if url.starts_with("http://") {
		&url[7..]
	} else {
		&url
	};

	// Grab credentials (if they exist), and reset slice of url after credentials.
	let credentials = if let Some(idx) = url.find('@') {
		// This ensures a port isn't being lost.
		let pre_split = &url[..idx];

		let username = &pre_split[..pre_split.find(':').ok_or(Error::InvalidCredentialsInURL)?];
		let password = &pre_split[pre_split.find(':').ok_or(Error::InvalidCredentialsInURL)? + 1..];

		url = &url[idx + 1..];

		Some((username.into(), password.into()))
	} else {
		None
	};

	// Grab host and resource.
	let (host, resource) = if let Some(idx) = url.find('/') {
		(&url[..idx], &url[idx..])
	} else {
		// Default resource is '/'.
		(url, "/")
	};

	// Add a port if one doesn't exist.
	let fin_host = if !host.contains(':') {
		let mut temp = host.to_string();

		temp += if https {
			":443"
		} else {
			":80"
		};

		temp
	} else {
		host.to_string()
	};

	// Return Url to reduce legible complexity.
	Ok(Url {
		https,
		credentials,
		host: fin_host,
		resource: resource.to_string(),
	})
}

/// This just ensures host is ASCII.
fn ensure_ascii(host: String) -> Result<String, Error> {
	if host.is_ascii() {
		Ok(host)
	} else {
		#[cfg(not(feature = "punycode"))]
		{ Err(Error::HostNotASCII) }

		#[cfg(feature = "punycode")]
		{
			let mut res = String::new();

			// Splits parts of domain in order to preserve any ASCII parts.
			for s in host.split('.') {
				if s.is_ascii() {
					res += s;
				} else {
					// The punycode crate does not prefix.
					match punycode::encode(s) {
						Ok(s) => res += &("xn--".to_owned() + &s),
						Err(_) => return Err(Error::DNSOverflow),
					}
				}

				res += ".";
			}

			// Remove trailing period.
			res.truncate(res.len() - 1);

			Ok(res)
		}
	}
}

/// Generate request head byte vector.
fn gen_head(request: &ClientRequest) -> Result<Vec<u8>, Error> {
	let mut head = vec![];

	writeln!(head, "{} {} HTTP/1.1\r\nHost: {}\r", request.method, request.url.resource, request.url.host)?;

	if request.headers.get("User-Agent").is_none() {
		writeln!(head, "User-Agent: slimweb\r")?;
	}

	// Write custom headers prior to necessary headers (based on provided info).
	for (k, v) in &request.headers {
		writeln!(head, "{}: {}\r", k, v)?;
	}

	if let Some(creds) = &request.url.credentials {
		if request.headers.get("Authorization").is_some() {
			println!("You passed an authorization header AND credentials in the URL (header is default). Pick one.");
		} else {
			writeln!(head, "Authorization: {}\r", base64::encode(&format!("{}:{}", creds.0, creds.1)))?;
		}
	}

	if request.chunk_size.is_none() {
		if let Some(body) = &request.body {
			let body_vec: Vec<u8> = body.into();

			writeln!(head, "Content-Length: {}\r", body_vec.len())?;
		}
	} else {
		writeln!(head, "Transfer-Encoding: chunked\r")?;
	}

	if request.compression {
		writeln!(head, "Content-Encoding: gzip\r")?;
	}

	writeln!(head, "\r")?;

	Ok(head)
}
