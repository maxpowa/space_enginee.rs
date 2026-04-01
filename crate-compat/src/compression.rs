//! MyCompression equivalent — gzip with a 4-byte LE uncompressed-length prefix.
//!
//! Mirrors `VRage.MyCompression.Compress` / `Decompress` from Space Engineers.

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::{self, Read, Write};

/// Compress `data` using gzip with a 4-byte little-endian uncompressed-length prefix.
///
/// Format: `[u32le uncompressed_len] [gzip bytes...]`
///
/// This matches the C# `VRage.MyCompression.Compress` format.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let uncompressed_len = data.len() as u32;

    let mut gz_buf = Vec::new();
    let mut encoder = GzEncoder::new(&mut gz_buf, Compression::default());
    encoder.write_all(data).expect("gzip write failed");
    encoder.finish().expect("gzip finish failed");

    let mut output = Vec::with_capacity(4 + gz_buf.len());
    output.extend_from_slice(&uncompressed_len.to_le_bytes());
    output.extend_from_slice(&gz_buf);
    output
}

/// Decompress data produced by [`compress`].
///
/// Reads the 4-byte little-endian uncompressed length prefix, then gzip-decompresses
/// the remainder. Returns `Err` if the data is too short or decompression fails.
pub fn decompress(data: &[u8]) -> io::Result<Vec<u8>> {
    if data.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "compressed data too short for length prefix",
        ));
    }

    let uncompressed_len =
        u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    let mut output = Vec::with_capacity(uncompressed_len);
    GzDecoder::new(&data[4..]).read_to_end(&mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let original = b"Hello, Space Engineers!";
        let compressed = compress(original);

        // First 4 bytes are the LE uncompressed length
        let stored_len = u32::from_le_bytes([
            compressed[0],
            compressed[1],
            compressed[2],
            compressed[3],
        ]);
        assert_eq!(stored_len as usize, original.len());

        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn round_trip_empty() {
        let original: &[u8] = b"";
        let compressed = compress(original);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn decompress_too_short() {
        assert!(decompress(&[0, 1, 2]).is_err());
    }
}
