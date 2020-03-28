use std::io::Write;

#[cfg(feature = "client")]
use std::{
	io::{
		Read, BufReader,
		Result as IoResult,
	},
	fs::File,
	path::Path,
};

#[cfg(feature = "server")]
use crate::Error;



#[derive(Debug, Clone, PartialEq)]
struct Part {
	name: String,
	filename: Option<String>,
	content_type: Option<String>,
	data: Vec<u8>,
}

impl Into<Vec<u8>> for Part {
	fn into(self) -> Vec<u8> {
		let mut part_data = Vec::new();

		write!(part_data, "Content-Disposition: form-data; name={}", self.name).unwrap();

		if let Some(filename) = self.filename {
			writeln!(part_data, "; filename={}\r", filename).unwrap();
		} else {
			writeln!(part_data, "\r").unwrap();
		}

		if let Some(ref content_type) = self.content_type {
			writeln!(part_data, "Content-Type: {}\r", content_type).unwrap();
		}

		writeln!(part_data, "\r").unwrap();

		part_data.extend::<Vec<u8>>(self.data);

		writeln!(part_data, "\r").unwrap();

		if self.content_type.is_some() {
			writeln!(part_data, "\r").unwrap();
		}

		part_data
	}
}

impl Into<Vec<u8>> for &Part {
	fn into(self) -> Vec<u8> {
		let mut part_data = Vec::new();

		write!(part_data, "Content-Disposition: form-data; name={}", self.name).unwrap();

		if let Some(filename) = &self.filename {
			writeln!(part_data, "; filename={}\r", filename).unwrap();
		} else {
			writeln!(part_data, "\r").unwrap();
		}

		if let Some(ref content_type) = self.content_type {
			writeln!(part_data, "Content-Type: {}\r", content_type).unwrap();
		}

		writeln!(part_data, "\r").unwrap();

		part_data.extend::<Vec<u8>>(self.data.clone());

		writeln!(part_data, "\r").unwrap();

		if self.content_type.is_some() {
			writeln!(part_data, "\r").unwrap();
		}

		part_data
	}
}

/// Multipart body created by user or server.
#[derive(Debug, Clone, PartialEq)]
pub struct Multipart {
	boundary: String,
	parts: Vec<Part>,
}

impl Into<Vec<u8>> for Multipart {
	fn into(self) -> Vec<u8> {
		let mut multi_data = Vec::new();

		for part in self.parts {
			writeln!(multi_data, "--{}\r", self.boundary).unwrap();

			multi_data.extend::<Vec<u8>>(part.into());
		}

		write!(multi_data, "--{}--", self.boundary).unwrap();

		multi_data
	}
}

impl Into<Vec<u8>> for &Multipart {
	fn into(self) -> Vec<u8> {
		let mut multi_data = Vec::new();

		for part in &self.parts {
			writeln!(multi_data, "--{}\r", self.boundary).unwrap();

			multi_data.extend::<Vec<u8>>(part.into());
		}

		write!(multi_data, "--{}--", self.boundary).unwrap();

		multi_data
	}
}

#[cfg(feature = "client")]
impl Multipart {
	/// Create new multipart body.
	pub fn new<S: Into<String>>(boundary: S) -> Multipart {
		Multipart {
			boundary: boundary.into(),
			parts: Vec::new(),
		}
	}

	/// Adds implied text_plain field to multipart body.
	pub fn text_part<S: Into<String>, D: Into<Vec<u8>>>(mut self, name: S, text: D) -> Multipart {
		self.parts.push(Part {
			name: name.into(),
			filename: None,
			content_type: None,
			data: text.into(),
		});

		self
	}

	/// Adds file to multipart body and guesses the MIME type.
	pub fn file_part<S: Into<String>, F: AsRef<Path>>(mut self, name: S, path: F) -> IoResult<Multipart> {
		let file = File::open(path.as_ref())?;
		let mut file_reader = BufReader::new(file);
		let mut file_data = Vec::new();

		file_reader.read_to_end(&mut file_data)?;

		let content_type = mime_guess::from_path(path.as_ref()).first_or_octet_stream();
		let filename = path.as_ref().file_name().and_then(|filename| Some(filename.to_str()?.to_string()));

		self.parts.push(Part {
			name: name.into(),
			filename,
			content_type: Some(content_type.to_string()),
			data: file_data,
		});

		Ok(self)
	}
}

#[cfg(feature = "server")]
pub(crate) fn from_bytes(boundary: &str, bytes: Vec<u8>) -> Result<Multipart, Error> {
	let mut multi = Multipart {
		boundary: boundary.to_string(),
		parts: Vec::new(),
	};

	let mut lines = Vec::new();

	{ // Prepare parts for processing by line.
		let mut line = Vec::new();

		for byte in bytes {
			if byte == b'\n' {
				if line.ends_with(&[b'\r']) {
					line.truncate(line.len() - 1);
				}

				lines.push(line.clone());

				line.clear();
			} else {
				line.push(byte);
			}
		}
	}

	let mut new_part: Option<Part> = None;
	let mut part_body = false;

	for line in lines {
		// Check for boundary and new part.
		if &line[line.len() - boundary.len()..] == boundary.as_bytes() && new_part.is_some() {
			multi.parts.push(new_part.clone().unwrap());

			new_part = None;
			part_body = false;

			continue;
		}

		if new_part.is_none() {
			new_part = Some(Part {
				name: String::new(),
				filename: None,
				content_type: None,
				data: Vec::new(),
			});
		}

		if line.is_empty() {
			part_body = true;

			continue;
		}

		if !part_body {
			if let Ok(text) = std::str::from_utf8(&line) {
				let split_text: Vec<&str> = text.split(':').collect();

				if split_text[0] == "Content-Disposition" {
					let split_text: Vec<&str> = split_text[1].trim().split(';').collect();

					for split in split_text {
						let split = split.trim();

						if split != "form-data" {
							if let Some(ref mut part) = new_part {
								if split.starts_with("name=") {
									part.name = split[5..].to_string();
								}

								if split.starts_with("filename=") {
									part.filename = Some(split[9..].to_string());
								}
							}
						}
					}
				} else if split_text[0] == "Content-Type" {
					if let Some(ref mut part) = new_part {
						part.content_type = Some(split_text[1].trim().to_string());
					}
				}
			}
		} else if let Some(ref mut part) = new_part {
			part.data.extend(line);
		}
	}

	Ok(multi)
}
