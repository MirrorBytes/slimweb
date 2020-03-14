use std::{
	thread,
	io::{
		Write, BufReader,
		Result as IoResult,
	},
	net::TcpStream,
};

use crate::{
	stream::{ self, Stream },
	Server, ServerResponse, StatusInfo,
};



#[test]
fn test_basic() -> IoResult<()> {
	thread::spawn(|| {
		Server::new("localhost:8080").unwrap()
			.add_handler("GET", "/", |_| {
				ServerResponse::new(200)
			})
			.run()
	});

	let mut client = TcpStream::connect("localhost:8080")?;

	writeln!(client, "GET / HTTP/1.1\r\nHost: localhost\r\n\r")?;

	let resp = stream::process_lines(&mut Stream::Http(BufReader::new(client)))?;

	match resp.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	Ok(())
}
