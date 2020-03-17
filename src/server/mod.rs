use std::{
	net::{ ToSocketAddrs, TcpListener },
	time::{ Instant, Duration },
	io::{
		BufReader, Write,
		Result as IoResult,
	},
	sync::{ Arc, Mutex },
	collections::HashMap,
};
#[cfg(feature = "tls")]
use std::fs::File;

#[cfg(feature = "tls")]
use rustls::{
	self,
	ServerConfig, NoClientAuth, KeyLogFile, StreamOwned, ServerSession,
};

use crate::{
	error::Error,
	stream::{
		self,
		Stream, Chunked, Compressed,
	},
	GeneralInfo, StatusInfo,
};

mod request;
mod response;
#[cfg(test)] mod tests;

use request::ServerRequest;
pub use self::response::ServerResponse;



/// Handler function defined by user.
pub type ServerHandler = Box<dyn Fn(&ServerRequest) -> Result<ServerResponse, Error> + Send + Sync>;

/// Handler function defined by user.
/// Think of these as header tests to ensure everything's fine prior to a potentially large body being received.
/// Returns status code (should be 100 if good, or the proper status code for any error), and message if one is needed.
pub type ExpectHandler = Box<dyn Fn(&GeneralInfo) -> Result<(i32, Option<String>), Error> + Send + Sync>;

/// Generic HTTP 1.1 Server.
pub struct Server {
	listener: TcpListener,
	handlers: Arc<Mutex<HashMap<(String, String), ServerHandler>>>,
	expect_handlers: Arc<Mutex<Vec<ExpectHandler>>>,

	#[cfg(feature = "tls")]
	tls_config: Option<Arc<ServerConfig>>,

	// deadline, reset
	deadline: Option<(Instant, Instant)>,
}

impl Server {
	/// Creates new Server with desired listening host.
	pub fn new<A: ToSocketAddrs>(host: A) -> Result<Server, Error> {
		let listener = TcpListener::bind(host)?;

		Ok(Server {
			listener,
			handlers: Arc::new(Mutex::new(HashMap::new())),
			expect_handlers: Arc::new(Mutex::new(Vec::new())),

			#[cfg(feature = "tls")]
			tls_config: None,

			deadline: None,
		})
	}

	/// Enables TLS encryption for every connection.
	#[cfg(feature = "tls")]
	pub fn tls(mut self, cert: &str, key: &str) -> Server {
		// TODO: Offer client authentication & OCSP.
		let mut config = ServerConfig::new(NoClientAuth::new());
		config.key_log = Arc::new(KeyLogFile::new());

		let file = File::open(cert).expect("Can't open cert file");
		let mut reader = BufReader::new(file);
		let certs = rustls::internal::pemfile::certs(&mut reader).expect("Unable to load certs");

		let rsa = {
			let file = File::open(key).expect("Can't open private key file");
			let mut reader = BufReader::new(file);

			rustls::internal::pemfile::rsa_private_keys(&mut reader).expect("File contains invalid rsa key")
		};
		let pkcs8 = {
			let file = File::open(key).expect("Can't open private key file");
			let mut reader = BufReader::new(file);

			rustls::internal::pemfile::pkcs8_private_keys(&mut reader).expect("File contains invalid pkcs8 key")
		};

		if !pkcs8.is_empty() {
			config.set_single_cert(certs, pkcs8[0].clone()).expect("Bad cert/private key");
		} else {
			assert!(!rsa.is_empty());

			config.set_single_cert(certs, rsa[0].clone()).expect("Bad cert/private key");
		}

		self.tls_config = Some(Arc::new(config));

		self
	}

	/// Adds a user defined handler to server.
	/// Handlers match method and resource, and call defined function.
	pub fn add_handler<S: Into<String>>(self, method: S, route: S, handler: impl Fn(&ServerRequest) -> Result<ServerResponse, Error> + 'static + Send + Sync) -> Server {
		self.handlers
			.lock().unwrap()
			.insert((method.into(), route.into()), Box::new(handler));

		self
	}

	/// Adds a user defined expectation handler to server.
	/// Used exclusively for Expect: 100-Continue handling.
	pub fn add_expect_handler(self, handler: impl Fn(&GeneralInfo) -> Result<(i32, Option<String>), Error> + 'static + Send + Sync) -> Server {
		self.expect_handlers
			.lock().unwrap()
			.push(Box::new(handler));

		self
	}

	/// Sets deadline for request.
	pub fn set_deadline(mut self, time: u64) -> Server {
		self.deadline = Some((Instant::now() + Duration::from_secs(time), Instant::now()));

		self
	}

	/// Start server loop, and begin handling requests.
	pub fn run(&mut self) -> IoResult<()> {
		let local_addr = self.listener.local_addr()?;
		info!("Server running on: {}:{}", local_addr.ip().to_string(), local_addr.port());

		let handlers = self.handlers.clone();
		let expect_handlers = self.expect_handlers.clone();

		for stream in self.listener.incoming() {
			let mut tcp;

			#[cfg(feature = "tls")]
			{
				if self.tls_config.is_some() {
					tcp = Stream::HttpsServer(BufReader::new(Box::new(StreamOwned::new(ServerSession::new(&self.tls_config.as_ref().unwrap()), stream?))));
				} else {
					tcp = Stream::Http(BufReader::new(stream?));
				}
			}

			#[cfg(not(feature = "tls"))]
			{
				tcp = Stream::Http(BufReader::new(stream?));
			}

			let info = stream::process_lines(&mut tcp)?;

			let mut continue_100 = false;
			let mut continue_code = 100;
			let mut continue_msg = None;

			let expect_handlers = expect_handlers.lock().unwrap();

			// Check for 100-Continue
			if info.headers.get("Expect").is_some() && !expect_handlers.is_empty() {
				continue_100 = true;

				for handler in expect_handlers.iter() {
					let (code, message) = handler(&info)?;

					if code != 100 {
						continue_code = code;
						continue_msg = message;

						break;
					}
				}
			}

			if continue_100 {
				let mut resp = ServerResponse::new(continue_code)?;

				if continue_code == 100 {
					let resp_vec: Vec<u8> = resp.into();

					// TODO: This isn't following deadlines. That needs to change.
					tcp.write_all(&resp_vec)?;
					tcp.flush()?;

					process_request(&mut tcp, info, &mut self.deadline, handlers.clone())?;
				} else { // something didn't pass expectations
					if let Some(msg) = continue_msg {
						resp = resp.set_body(msg.as_str());
					}

					let resp_vec: Vec<u8> = resp.into();

					// TODO: This isn't following deadlines. That needs to change.
					tcp.write_all(&resp_vec)?;
					tcp.flush()?;
				}
			} else {
				process_request(&mut tcp, info, &mut self.deadline, handlers.clone())?;
			}
		}

		Ok(())
	}
}

fn process_request(stream: &mut Stream, info: GeneralInfo, deadline: &mut Option<(Instant, Instant)>, handlers: Arc<Mutex<HashMap<(String, String), ServerHandler>>>) -> Result<(), Error> {
	let req = ServerRequest::new(stream, info, deadline)?;

	if let StatusInfo::Request(method, resource) = req.info.clone().status {
		if let Some(handler) = handlers.lock().unwrap().get(&(method, resource)) {
			let mut resp = handler(&req)?;

			if let Some(body) = resp.body.clone() {
				let body: Vec<u8> = body.into();

				if resp.chunk_size.is_some() {
					resp.info.headers.insert("Transfer-Encoding".into(), "chunked".into());
				} else {
					resp.info.headers.insert("Content-Length".into(), body.len().to_string());
				}

				if stream::check_accept(&req.info.headers) && resp.compression_level.is_some() {
					resp.info.headers.insert("Content-Encoding".into(), "gzip".into());
				}
			}

			let mut head: Vec<u8> = resp.info.into();
			writeln!(head, "\r").unwrap();

			// TODO: This isn't following deadlines. That needs to change.
			stream.write_all(&head)?;
			stream.flush()?;

			if let Some(body) = resp.body {
				let body: Vec<u8> = body.into();

				let mut chunked = Chunked::new(
					stream,
					resp.chunk_size,
					resp.chunk_size.is_some()
				);

				let mut compressed = if stream::check_accept(&req.info.headers) && resp.compression_level.is_some() {
					Compressed::new(
						&mut chunked,
						resp.compression_level,
						Some(&body),
						true
					)
				} else {
					Compressed::new(
						&mut chunked,
						None,
						None,
						false
					)
				};

				stream::write_all_until(&mut compressed, &body, deadline)?;
			}
		} else {
			warn!("No handler available for this resource.");
		}
	}

	Ok(())
}
