use std::{
	net::TcpStream,
	time::{ Instant, Duration },
	io::{
		BufReader, BufRead, Read, Write,
		Result as IoResult,
		Error as IoError,
		ErrorKind,
	},
	collections::HashMap,
};
#[cfg(feature = "client")]
use std::{
	net::{ ToSocketAddrs, SocketAddr },
	fmt::Display,
};

#[cfg(feature = "tls")]
use rustls::{ StreamOwned, ClientSession, ServerSession };

#[cfg(feature = "compress")]
use flate2::{
	bufread::{ GzDecoder, GzEncoder },
	Compression,
};

use crate::{
	error::Error,
	StatusInfo, GeneralInfo,
};



trait GetRefMut {
	fn get_ref(&self) -> &Stream;

	fn get_mut(&mut self) -> &mut Stream;
}

pub(crate) enum Stream {
	Http(BufReader<TcpStream>),
	// Boxing for size variation.
	#[cfg(feature = "tls")]
	HttpsClient(BufReader<Box<StreamOwned<ClientSession, TcpStream>>>),
	// Boxing for size variation.
	#[cfg(feature = "tls")]
	HttpsServer(BufReader<Box<StreamOwned<ServerSession, TcpStream>>>),
}

implread!(Stream, (Http, #[cfg(feature = "tls")] HttpsClient, #[cfg(feature = "tls")] HttpsServer));
implbufread!(Stream, (Http, #[cfg(feature = "tls")] HttpsClient, #[cfg(feature = "tls")] HttpsServer));
implwrite!(Stream, (Http .get_mut(), #[cfg(feature = "tls")] HttpsClient .get_mut(), #[cfg(feature = "tls")] HttpsServer .get_mut()));

impl Stream {
	pub(crate) fn get_ref(&self) -> &TcpStream {
		match self {
			Stream::Http(s) => s.get_ref(),
			#[cfg(feature = "tls")]
			Stream::HttpsClient(s) => s.get_ref().get_ref(),
			#[cfg(feature = "tls")]
			Stream::HttpsServer(s) => s.get_ref().get_ref(),
		}
	}
}



pub(crate) struct ChunkedReader<'r> {
	stream: &'r mut Stream,
	buffer: Vec<u8>,
	consumed: usize,
	remaining: usize,
	eof: bool,
}

impl<'r> BufRead for ChunkedReader<'r> {
	fn fill_buf(&mut self) -> IoResult<&[u8]> {
		if self.buffer.len() == self.consumed && !(self.remaining == 0 && self.eof) {
			if self.remaining == 0 {
				self.remaining = self.read_chunk_size()?;

				if self.remaining == 0 {
					self.eof = true;
				}
			}

			self.buffer.resize(std::cmp::min(self.remaining, 64 * 1024), 0);
			self.stream.read_exact(&mut self.buffer)?;
			self.consumed = 0;
			self.remaining -= self.buffer.len();

			if self.remaining == 0 && !read_line_ending(self.get_mut())? {
				self.buffer.clear();
				self.eof = true;

				return Err(IoError::new(ErrorKind::Other, "Problem decoding response chunk"));
			}
		}

		Ok(&self.buffer[self.consumed..])
	}

	fn consume(&mut self, amt: usize) {
		self.consumed = std::cmp::min(self.consumed + amt, self.buffer.len());
	}
}

impl<'r> Read for ChunkedReader<'r> {
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		let n = self.fill_buf()?.read(buf)?;
		self.consume(n);

		Ok(n)
	}
}

impl<'r> ChunkedReader<'r> {
	fn new(stream: &'r mut Stream) -> ChunkedReader<'r> {
		ChunkedReader {
			stream,
			buffer: vec![],
			remaining: 0,
			consumed: 0,
			eof: false,
		}
	}

	fn get_ref(&self) -> &Stream {
		&self.stream
	}

	fn get_mut(&mut self) -> &mut Stream {
		&mut self.stream
	}

	fn read_chunk_size(&mut self) -> IoResult<usize> {
		read_line(&mut self.stream, &mut self.buffer, 128)?;

		if self.buffer.is_empty() {
			return Err(ErrorKind::UnexpectedEof.into());
		}

		self.buffer
			.iter()
			.position(|&b| b == b';')
			.map_or_else(|| std::str::from_utf8(&self.buffer), |idx| std::str::from_utf8(&self.buffer[..idx]))
			.map_err(|_| Error::ChunkError)
			.and_then(|chunk| usize::from_str_radix(chunk, 16).map_err(|_| Error::ChunkError))
			.map_err(|e| e.into())
	}
}

pub(crate) struct ChunkedWriter<'r> {
	stream: &'r mut Stream,
	chunk_size: usize,
	buffer: Vec<u8>,
}

impl<'r> ChunkedWriter<'r> {
	fn new(stream: &'r mut Stream, chunk_size: usize) -> ChunkedWriter<'r> {
		ChunkedWriter {
			stream,
			chunk_size,
			buffer: vec![],
		}
	}

	fn get_ref(&self) -> &Stream {
		&self.stream
	}

	fn get_mut(&mut self) -> &mut Stream {
		&mut self.stream
	}
}

impl<'r> Write for ChunkedWriter<'r> {
	fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
		self.buffer.write_all(buf)?;

		while self.buffer.len() >= self.chunk_size {
			let rest = {
				let (to_send, rest) = self.buffer.split_at_mut(self.chunk_size);

				write_to_payload(&mut self.stream, to_send)?;

				rest.to_vec()
			};

			self.buffer = rest;
		}

		Ok(buf.len())
	}

	fn flush(&mut self) -> IoResult<()> {
		if self.buffer.is_empty() {
			return Ok(());
		}

		write_to_payload(&mut self.stream, &self.buffer)?;

		self.buffer.clear();

		Ok(())
	}
}

pub(crate) enum Chunky<'r> {
	Read(ChunkedReader<'r>),
	Write(ChunkedWriter<'r>),
}

impl<'r> Read for Chunky<'r> {
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		match self {
			Chunky::Read(s) => s.read(buf),
			Chunky::Write(_) => Ok(0), // This should never be called.
		}
	}
}

impl<'r> Write for Chunky<'r> {
	fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
		match self {
			Chunky::Read(_) => Ok(0),// This should never be called.
			Chunky::Write(s) => s.write(buf),
		}
	}

	fn flush(&mut self) -> IoResult<()> {
		match self {
			Chunky::Read(_) => Ok(()),// This should never be called.
			Chunky::Write(s) => s.flush(),
		}
	}
}

impl<'r> BufRead for Chunky<'r> {
	fn fill_buf(&mut self) -> IoResult<&[u8]> {
		match self {
			Chunky::Read(s) => s.fill_buf(),
			Chunky::Write(_) => Ok(&[]), // This should never be called.
		}
	}

	fn consume(&mut self, amt: usize) {
		match self {
			Chunky::Read(s) => s.consume(amt),
			Chunky::Write(_) => (), // This should never be called.
		}
	}
}

implgets!(Chunky, 'r, (Read, Write));

pub(crate) enum Chunked<'r> {
	Non(&'r mut Stream),
	Is(Chunky<'r>),
}

implread!(Chunked, 'r, (Non, Is));
implbufread!(Chunked, 'r, (Non, Is));
implwrite!(Chunked, 'r, (Non, Is));

impl<'r> Chunked<'r> {
	pub fn new(stream: &'r mut Stream, chunk_size: Option<usize>, chunked: bool) -> Chunked<'r> {
		if chunked {
			if let Some(size) = chunk_size {
				Chunked::Is(Chunky::Write(ChunkedWriter::new(stream, size)))
			} else {
				Chunked::Is(Chunky::Read(ChunkedReader::new(stream)))
			}
		} else {
			Chunked::Non(stream)
		}
	}
}

impl<'r> GetRefMut for Chunked<'r> {
	fn get_ref(&self) -> &Stream {
		match self {
			Chunked::Non(s) => &s,
			Chunked::Is(s) => s.get_ref(),
		}
	}

	fn get_mut(&mut self) -> &mut Stream {
		match self {
			Chunked::Non(s) => s,
			Chunked::Is(s) => s.get_mut(),
		}
	}
}



#[cfg(feature = "compress")]
pub(crate) enum Gzip<'r, Chunked> {
	De(GzDecoder<&'r mut Chunked>),
	En((&'r mut Chunked, GzEncoder<&'r [u8]>)),
}

#[cfg(feature = "compress")]
impl<'r> Read for Gzip<'r, Chunked<'r>> {
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		match self {
			Gzip::De(s) => s.read(buf),
			Gzip::En((p, _)) => p.read(buf), // This should never be called.
		}
	}
}

#[cfg(feature = "compress")]
impl<'r> Write for Gzip<'r, Chunked<'r>> {
	fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
		match self {
			Gzip::De(s) => s.write(buf), // This should never be called.
			Gzip::En((p, s)) => {
				let mut buffer = vec![];

				s.read_exact(&mut buffer)?;

				p.write(&buffer)
			},
		}
	}

	fn flush(&mut self) -> IoResult<()> {
		match self {
			Gzip::De(s) => s.flush(),
			Gzip::En((p, _)) => p.flush(),
		}
	}
}

#[cfg(feature = "compress")]
impl<'r> BufRead for Gzip<'r, Chunked<'r>> {
	fn fill_buf(&mut self) -> IoResult<&[u8]> {
		match self {
			Gzip::De(s) => s.get_mut().fill_buf(),
			Gzip::En((p, _)) => p.get_mut().fill_buf(),
		}
	}

	fn consume(&mut self, amt: usize) {
		match self {
			Gzip::De(s) => s.get_mut().consume(amt),
			Gzip::En((p, _)) => p.get_mut().consume(amt),
		}
	}
}

#[cfg(feature = "compress")]
impl<'r> GetRefMut for Gzip<'r, Chunked<'r>> {
	fn get_ref(&self) -> &Stream {
		match self {
			Gzip::De(s) => s.get_ref().get_ref(),
			Gzip::En((p, _)) => p.get_ref(),
		}
	}

	fn get_mut(&mut self) -> &mut Stream {
		match self {
			Gzip::De(s) => s.get_mut().get_mut(),
			Gzip::En((p, _)) => p.get_mut(),
		}
	}
}



pub(crate) enum Compressed<'r> {
	Non(&'r mut Chunked<'r>),
	#[cfg(feature = "compress")]
	Is(Gzip<'r, Chunked<'r>>),
}

implread!(Compressed, 'r, (Non, #[cfg(feature = "compress")] Is));
implbufread!(Compressed, 'r, (Non, #[cfg(feature = "compress")] Is));
implwrite!(Compressed, 'r, (Non, #[cfg(feature = "compress")] Is));

impl<'r> Compressed<'r> {
	#[cfg(feature = "compress")]
	pub fn new(stream: &'r mut Chunked<'r>, comp_level: Option<u32>, data: Option<&'r [u8]>, compressed: bool) -> Compressed<'r> {
		if compressed {
			if let Some(level) = comp_level {
				Compressed::Is(Gzip::En((stream, GzEncoder::new(data.unwrap(), Compression::new(level)))))
			} else {
				Compressed::Is(Gzip::De(GzDecoder::new(stream)))
			}
		} else {
			Compressed::Non(stream)
		}
	}

	#[cfg(not(feature = "compress"))]
	pub(crate) fn new(stream: &'r mut Chunked<'r>, _: Option<u32>, _: Option<&'r [u8]>, _: bool) -> Compressed<'r> {
		Compressed::Non(stream)
	}
}

implgets!(Compressed, 'r, (Non, #[cfg(feature = "compress")] Is));



// -----------------------------------------------------------------------------------------------------------

#[cfg(feature = "client")]
pub(crate) fn connect<A: ToSocketAddrs + Display>(host: A, deadline: Option<(Instant, Instant)>) -> Result<TcpStream, Error> {
	let ips: Vec<SocketAddr> = host.to_socket_addrs()
		.map_err(|e| Error::ConnectionFailed(format!("{}", e)))?
		.collect();

	if ips.is_empty() {
		return Err(Error::ConnectionFailed(format!("No ip address for {}", host)));
	}

	let sock_addr = ips[0];

	if let Some((deadline, _)) = deadline {
		let now = Instant::now();

		Ok(TcpStream::connect_timeout(&sock_addr, deadline - now)?)
	} else {
		Ok(TcpStream::connect(sock_addr)?)
	}
}



fn write_until(stream: &mut Compressed<'_>, req: &[u8], deadline: &mut Option<(Instant, Instant)>) -> Result<usize, Error> {
	if let Some((line, reset)) = deadline {
		// Having a deadline guarantees a deadline_reset.
		if reset.elapsed() >= Duration::from_millis(250) {
			let now = Instant::now();

			if *line <= now {
				return Err(Error::Io(IoError::new(ErrorKind::TimedOut, "Connection timed out")));
			}

			stream
				.get_ref()
				.get_ref()
				.set_write_timeout(Some(*line - now))?;

			*deadline = Some((*line, Instant::now()));
		}
	}

	Ok(stream.write(req)?)
}

pub(crate) fn write_all_until(stream: &mut Compressed<'_>, mut req: &[u8], deadline: &mut Option<(Instant, Instant)>) -> Result<(), Error> {
	while !req.is_empty() {
		let n = write_until(stream, req, deadline)?;

		if n == 0 {
			return Err(Error::Io(IoError::new(ErrorKind::UnexpectedEof, "")));
		}

		req = &req[n..];
	}

	stream.flush()?;

	Ok(())
}



pub(crate) fn read_until(stream: &mut Compressed<'_>, buf: &mut [u8], deadline: &mut Option<(Instant, Instant)>) -> Result<usize, Error> {
	if let Some((line, reset)) = deadline {
		// Having a deadline guarantees a deadline_reset.
		if reset.elapsed() >= Duration::from_millis(250) {
			let now = Instant::now();

			if *line <= now {
				return Err(Error::Io(IoError::new(ErrorKind::TimedOut, "Connection timed out")));
			}

			stream
				.get_ref()
				.get_ref()
				.set_read_timeout(Some(*line - now))?;

			*deadline = Some((*line, Instant::now()));
		}
	}

	match stream.read(buf) {
		Ok(size) => Ok(size),
		Err(ref e) if is_close_notify(e) => Ok(0),
		Err(e) => Err(Error::Io(e)),
	}
}

pub(crate) fn read_to_end_until(stream: &mut Compressed<'_>, body: &mut Vec<u8>, content_length: Option<usize>, deadline: &mut Option<(Instant, Instant)>) -> Result<(), Error> {
	let mut buf = [0; 1024];

	let mut cur_len = 0;

	loop {
		let n = read_until(stream, &mut buf, deadline)?;

		body.extend_from_slice(&buf[..n]);

		if let Some(length) = content_length {
			cur_len += n;

			if length == cur_len {
				break;
			}
		}

		if n == 0 {
			break;
		}
	}

	Ok(())
}



pub(crate) fn process_lines(stream: &mut Stream) -> Result<GeneralInfo, Error> {
	let mut buf = vec![];

	read_line(stream, &mut buf, 8 * 1024)?;

	// Get status line (if one exists).
	let status = parse_status_line(&mut buf)?;
	let mut headers = HashMap::new();

	loop {
		read_line(stream, &mut buf, 8 * 1024)?;

		// We've hit the body.
		if buf.is_empty() {

			break;
		}

		if let Some(parsed) = parse_header(&buf) {
			headers.insert(parsed.0, parsed.1);
		}
	}

	Ok(GeneralInfo {
		status,
		headers,
	})
}

fn parse_status_line(line: &mut Vec<u8>) -> Result<StatusInfo, Error> {
	let mut split = line.split(|&b| b == b' ').filter(|x| !x.is_empty());

	let method = split.nth(0).unwrap();

	if let Some(code) = split.nth(0) {
		if let Ok(code) = std::str::from_utf8(code) {
			if let Ok(code) = code.parse::<i32>() { // server response
				if let Some(reason) = split.next() {
					if let Ok(reason) = std::str::from_utf8(reason) {
						return Ok(StatusInfo::Response(code, reason.to_string()));
					}
				}
			} else { // client request
				// code is technically resource here.
				// TODO: Clean this up.
				return Ok(StatusInfo::Request(std::str::from_utf8(method).unwrap().to_string(), code.to_string()));
			}
		}
	}

	Err(Error::NoStatusLineInResponse)
}

fn parse_header(line: &[u8]) -> Option<(String, String)> {
	if let Some(idx) = line.iter().position(|&x| x == b':') {
		let header = &line[..idx];
		let val = if line[idx..].starts_with(&[b' ']) {
			&line[idx + 2..]
		} else {
			&line[idx + 1..]
		};

		if let Ok(header) = std::str::from_utf8(header) {
			if let Ok(val) = std::str::from_utf8(val) {
				return Some((header.to_string(), val.trim().to_string()));
			}
		}
	}

	None
}



pub(crate) fn check_encodings(headers: &HashMap<String, String>) -> (bool, bool) {
	let mut compression = false;
	let mut chunking = false;

	if let Some(content) = headers.get("Content-Encoding") {
		compression = content
			.split(',')
			.map(|s| s.trim())
			.any(|s| s.eq_ignore_ascii_case("gzip"));
	}

	if let Some(transfer) = headers.get("Transfer-Encoding") {
		chunking = transfer
			.split(',')
			.map(|s| s.trim())
			.any(|s| s.eq_ignore_ascii_case("chunked"));
	}

	(compression, chunking)
}

#[cfg(feature = "server")]
pub(crate) fn check_accept(headers: &HashMap<String, String>) -> bool {
	if let Some(accept) = headers.get("Accept-Encoding") {
		return accept
			.split(',')
			.map(|s| s.trim())
			.any(|s| s.eq_ignore_ascii_case("gzip"));
	}

	false
}



// -----------------------------------------------------------------------------------------------------------
// Helper functions

fn read_line(stream: &mut Stream, mut buf: &mut Vec<u8>, max: u64) -> Result<usize, Error> {
	buf.clear();

	let n = stream.take(max).read_until(b'\n', &mut buf)?;

	if buf.ends_with(&[b'\r', b'\n']) {
		buf.truncate(buf.len() - 2);
	} else if buf.ends_with(&[b'\n']) {
		buf.truncate(buf.len() - 1);
	} else {
		return Err(Error::Io(ErrorKind::UnexpectedEof.into()));
	}

	Ok(n)
}

fn read_line_ending(stream: &mut Stream) -> IoResult<bool> {
	let mut b = [0];

	stream.read_exact(&mut b)?;

	if &b == b"\r" {
		stream.read_exact(&mut b)?;
	}

	Ok(&b == b"\n")
}

fn write_to_payload(source: &mut Stream, data: &[u8]) -> IoResult<()> {
	writeln!(source, "{:x}\r", data.len())?;

	source.write_all(&data)?;

	writeln!(source, "\r")?;

	Ok(())
}

fn is_close_notify(err: &IoError) -> bool {
	if err.kind() != ErrorKind::ConnectionAborted {
		return false;
	}

	if let Some(msg) = err.get_ref() {
		return msg.description().contains("CloseNotify");
	}

	false
}
