#[macro_export]
/// Generic impl Read macro.
///
/// Example:
/// ```ignore
/// implread!(Stream, (Http, #[cfg(feature = "tls")] Https));
/// ```
/// Becomes:
/// ```ignore
/// impl Read for Stream {
///		fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
///			match self {
///				Stream::Http(s) => s.read(buf),
/// 			#[cfg(feature = "tls")] Stream::Https(s) => s.read(buf),
///			}
///		}
///	}
/// ```
macro_rules! implread {
	($name:ident, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident),*)) => {
		impl Read for $name {
			fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.read(buf),)*
				}
			}
		}
	};

	($name:ident, $life:lifetime, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident),*)) => {
		impl<$life> Read for $name<$life> {
			fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.read(buf),)*
				}
			}
		}
	};
}

#[macro_export]
/// Generic impl BufRead macro.
///
/// Example:
/// ```ignore
/// implbufread!(Stream, (Http, #[cfg(feature = "tls")] Https));
/// ```
/// Becomes:
/// ```ignore
/// impl BufRead for Stream {
///		fn fill_buf(&mut self) -> IoResult<&[u8]> {
///			match self {
///				Stream::Http(s) => s.fill_buf(),
/// 			#[cfg(feature = "tls")] Stream::Https(s) => s.fill_buf(),
///			}
///		}
///
///		fn consume(&mut self, amt: usize) {
///			match self {
///				Stream::Http(s) => s.consume(amt),
/// 			#[cfg(feature = "tls")] Stream::Https(s) => s.consume(amt),
///			}
///		}
///	}
/// ```
macro_rules! implbufread {
	($name:ident, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident),*)) => {
		impl BufRead for $name {
			fn fill_buf(&mut self) -> IoResult<&[u8]> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.fill_buf(),)*
				}
			}

			fn consume(&mut self, amt: usize) {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.consume(amt),)*
				}
			}
		}
	};

	($name:ident, $life:lifetime, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident),*)) => {
		impl<$life> BufRead for $name<$life> {
			fn fill_buf(&mut self) -> IoResult<&[u8]> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.fill_buf(),)*
				}
			}

			fn consume(&mut self, amt: usize) {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.consume(amt),)*
				}
			}
		}
	};
}

#[macro_export]
/// Generic impl Write macro.
///
/// Example:
/// ```ignore
/// implwrite!(Stream, (Http .get_mut(), #[cfg(feature = "tls")] Https .get_mut()));
/// ```
/// Becomes:
/// ```ignore
/// impl Write for Stream {
///		fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
///			match self {
///				Stream::Http(s) => s.get_mut().write(buf),
/// 			#[cfg(feature = "tls")] Stream::Https(s) => s.get_mut().write(buf),
///			}
///		}
///
///		fn flush(&mut self) -> IoResult<()> {
///			match self {
///				Stream::Http(s) => s.get_mut().flush(),
/// 			#[cfg(feature = "tls")] Stream::Https(s) => s.get_mut().flush(),
///			}
///		}
///	}
/// ```
macro_rules! implwrite {
	($name:ident, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident $(.$ex:ident())?),*)) => {
		impl Write for $name {
			fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s$(.$ex())?.write(buf),)*
				}
			}

			fn flush(&mut self) -> IoResult<()> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s$(.$ex())?.flush(),)*
				}
			}
		}
	};

	($name:ident, $life:lifetime, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident $(.$ex:ident())?),*)) => {
		impl<$life> Write for $name<$life> {
			fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s$(.$ex())?.write(buf),)*
				}
			}

			fn flush(&mut self) -> IoResult<()> {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s$(.$ex())?.flush(),)*
				}
			}
		}
	};
}

#[macro_export]
/// Impl get_ref and get_mut.
///
/// There are only lifetime affected enums that this applies to currently.
///
/// Example:
/// ```ignore
/// implgets!(Chunky, 'r, (Read, Write));
/// ```
/// Becomes:
/// ```ignore
/// impl<'r> Chunky<'r> {
///		fn get_ref(&self) -> &Stream {
///			match self {
///				Chunky::Read(s) => s.get_ref(),
/// 			Chunky::Write(s) => s.get_ref(),
///			}
///		}
///
///		fn get_mut(&mut self) -> &mut Stream {
///			match self {
///				Chunky::Read(s) => s.get_mut(),
/// 			Chunky::Write(s) => s.get_mut(),
///			}
///		}
///	}
/// ```
macro_rules! implgets {
	($name:ident, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident),*)) => {
		impl GetRefMut for $name {
			fn get_ref(&self) -> &Stream {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.get_ref(),)*
				}
			}

			fn get_mut(&mut self) -> &mut Stream {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.get_mut(),)*
				}
			}
		}
	};

	($name:ident, $life:lifetime, ($($(#[cfg($cfg:ident = $val:tt)])? $op:ident),*)) => {
		impl<$life> GetRefMut for $name<$life> {
			fn get_ref(&self) -> &Stream {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.get_ref(),)*
				}
			}

			fn get_mut(&mut self) -> &mut Stream {
				match self {
					$($(#[cfg($cfg = $val)])? $name::$op(s) => s.get_mut(),)*
				}
			}
		}
	};
}
