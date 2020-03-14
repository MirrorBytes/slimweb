use crate::error::Error;

mod request;
mod response;
#[cfg(test)] mod tests;



/// Create a GET Request.
pub fn get(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("GET", url)
}

/// Create a POST Request.
pub fn post(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("POST", url)
}

/// Create a PUT Request.
pub fn put(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("PUT", url)
}

/// Create a PATCH Request.
pub fn patch(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("PATCH", url)
}

/// Create a DELETE Request.
pub fn delete(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("DELETE", url)
}

/// Create a HEAD Request.
pub fn head(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("HEAD", url)
}

/// Create a TRACE Request.
pub fn trace(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("TRACE", url)
}

/// Create a OPTIONS Request.
pub fn options(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("OPTIONS", url)
}

/// Create a CONNECT Request.
pub fn connect(url: &str) -> Result<request::ClientRequest, Error> {
	request::ClientRequest::new("CONNECT", url)
}
