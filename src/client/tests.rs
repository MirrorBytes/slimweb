use std::io::Result as IoResult;

use crate::{
	get, post, put, patch, delete,
	StatusInfo,
};



#[test]
#[cfg(feature = "tls")]
fn test_https() -> IoResult<()> {
	let resp = get("https://httpbin.org/get")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	Ok(())
}

#[test]
fn test_get() -> IoResult<()> {
	let resp = get("http://httpbin.org/get")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	Ok(())
}

#[test]
fn test_post() -> IoResult<()> {
	let resp = post("http://httpbin.org/post")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	Ok(())
}

#[test]
fn test_put() -> IoResult<()> {
	let resp = put("http://httpbin.org/put")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	Ok(())
}

#[test]
fn test_patch() -> IoResult<()> {
	let resp = patch("http://httpbin.org/patch")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	Ok(())
}

#[test]
fn test_delete() -> IoResult<()> {
	let resp = delete("http://httpbin.org/delete")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	Ok(())
}

#[test]
fn test_set_header() -> IoResult<()> {
	let resp = get("http://httpbin.org/get")?
		.set_header("some-random-header", "test")
		.send()?;

	assert!(
		resp.body
			.text()
			.contains("\"Some-Random-Header\": \"test\"")
	);

	Ok(())
}

#[test]
fn test_set_body() -> IoResult<()> {
	let resp = post("http://httpbin.org/post")?
		.set_body("Testing")
		.send()?;

	assert!(
		resp.body
			.text()
			.contains("\"data\": \"Testing\"")
	);

	Ok(())
}

#[test]
fn test_max_redirects() -> IoResult<()> {
	let resp = get("http://httpbin.org/redirect/5")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	let resp = get("http://httpbin.org/redirect/6")?
		.send();

	assert!(resp.is_err());

	Ok(())
}

#[test]
#[cfg(feature = "compress")]
fn test_decompression() -> IoResult<()> {
	let resp = get("http://httpbin.org/gzip")?
		.send()?;

	match resp.info.status {
		StatusInfo::Response(code, _) => assert_eq!(code, 200),
		_ => (),
	}

	assert!(
		resp.body
			.text()
			.contains("\"gzipped\": true")
	);

	Ok(())
}
