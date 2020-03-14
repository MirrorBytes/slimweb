use std::{
	net::{ ToSocketAddrs, TcpListener },
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
	StatusInfo,
};

mod request;
mod response;
#[cfg(test)] mod tests;

use request::ServerRequest;
pub use self::response::ServerResponse;



/// Handler function defined by user.
/// Has two passed parameters (head info of request, and body).
pub type ServerHandler = Box<dyn Fn(&ServerRequest) -> Result<ServerResponse, Error> + Send + Sync>;

/// Generic HTTP 1.1 Server.
pub struct Server {
	listener: TcpListener,
	handlers: Arc<Mutex<HashMap<(String, String), ServerHandler>>>,

	#[cfg(feature = "tls")]
	tls_config: Option<Arc<ServerConfig>>,

	compression: bool,
}

impl Server {
	/// Creates new Server with desired listening host.
	pub fn new<A: ToSocketAddrs>(host: A) -> Result<Server, Error> {
		let listener = TcpListener::bind(host)?;

		Ok(Server {
			listener,
			handlers: Arc::new(Mutex::new(HashMap::new())),

			#[cfg(feature = "tls")]
			tls_config: None,

			compression: false,
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

	/// Allow server to send compressed data.
	#[cfg(feature = "compress")]
	pub fn enable_compression(mut self) -> Server {
		self.compression = true;

		self
	}

	/// Start server loop, and begin handling requests.
	pub fn run(&self) -> IoResult<()> {
		let local_addr = self.listener.local_addr()?;
		info!("Server running on: {}:{}", local_addr.ip().to_string(), local_addr.port());

		let handlers = self.handlers.clone();

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

			let req = ServerRequest::new(&mut tcp)?;

			if let StatusInfo::Request(method, resource) = req.info.clone().status {
				if let Some(handler) = handlers.clone().lock().unwrap().get(&(method, resource)) {
					let mut resp = handler(&req)?;

					if let Some(body) = resp.body.clone() {
						let body: Vec<u8> = body.into();

						if resp.chunk_size.is_some() {
							resp.info.headers.insert("Transfer-Encoding".into(), "chunked".into());
						} else {
							resp.info.headers.insert("Content-Length".into(), body.len().to_string());
						}

						if stream::check_accept(&req.info.headers) && self.compression && resp.compression_level.is_some() {
							resp.info.headers.insert("Content-Encoding".into(), "gzip".into());
						}
					}

					let mut head: Vec<u8> = resp.info.into();
					writeln!(head, "\r").unwrap();

					tcp.write_all(&head)?;
					tcp.flush()?;

					if let Some(body) = resp.body {
						let body: Vec<u8> = body.into();

						let mut chunked = Chunked::new(
							&mut tcp,
							resp.chunk_size,
							resp.chunk_size.is_some()
						);

						let mut compressed = if stream::check_accept(&req.info.headers) && self.compression && resp.compression_level.is_some() {
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

						stream::write_all_until(&mut compressed, &body, &mut None)?;
					}
				} else {
					warn!("No handler available for this resource.");
				}
			}
		}

		Ok(())
	}
}
