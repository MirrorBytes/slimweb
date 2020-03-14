use std::{
	fmt, error,
	io::{
		ErrorKind,
		Error as IoError,
	},
};



/// Represents errors that can (and probably will) occur throughout this library.
#[derive(Debug)]
pub enum Error {
	/// Throws while parsing URL if an @ symbol is presented, and no colon is found (prior to port).
	InvalidCredentialsInURL,
	/// This only throws is the host is not ASCII, and punycode isn't being used.
	#[cfg(not(feature = "punycode"))]
	HostNotASCII,
	/// DNS can only resolve up to 63 bytes. This is thrown if it surpasses.
	#[cfg(feature = "punycode")]
	DNSOverflow,
	/// Throws if a requested URL is using HTTPS, and the tls feature is not enabled.
	TLSNotEnabled,
	/// Response does not contain a status line.
	NoStatusLineInResponse,
	/// Problem decoding chunk of response.
	ChunkError,
	/// The set number of max redirects (default 5) has been reached.
	MaxRedirectsHit,
	/// Redirect location header missing.
	NoLocationHeader,
	/// Occurs when host cannot be converted to SockAddr.
	ConnectionFailed(String),
	/// HTTP Status Code not recognized.
	HTTPStatusCodeNotRecognized,
	/// Any generic IO error.
	Io(IoError),
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::InvalidCredentialsInURL => write!(f, "URL contains invalid credentials format. Should look like: '[?]USERNAME:PASSWORD@HOST[?]'"),
			#[cfg(not(feature = "punycode"))]
			Error::HostNotASCII => write!(f, "Host not in ASCII format"),
			#[cfg(feature = "punycode")]
			Error::DNSOverflow => write!(f, "Requested host could not be converted to ASCII, too many bytes"),
			Error::TLSNotEnabled => write!(f, "Attempting to connect to secure URL without tls feature enabled"),
			Error::NoStatusLineInResponse => write!(f, "Response does not contain a status line"),
			Error::ChunkError => write!(f, "Problem decoding chunk of response"),
			Error::MaxRedirectsHit => write!(f, "Your request hit maximum number of redirects. You can increase this limit by using .set_max_redirects(usize)"),
			Error::NoLocationHeader => write!(f, "Redirect location header missing"),
			Error::ConnectionFailed(msg) => write!(f, "{}", msg),
			Error::HTTPStatusCodeNotRecognized => write!(f, "HTTP status code supplied is not supported or does not exist."),
			Error::Io(ioe) => write!(f, "Network error: {}", ioe),
		}
	}
}

impl error::Error for Error {
	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match self {
			Error::Io(err) => Some(err),
			_ => None,
		}
	}
}

impl From<IoError> for Error {
	fn from(err: IoError) -> Error {
		Error::Io(err)
	}
}

impl From<Error> for IoError {
	fn from(err: Error) -> IoError {
		IoError::new(ErrorKind::Other, err)
	}
}
