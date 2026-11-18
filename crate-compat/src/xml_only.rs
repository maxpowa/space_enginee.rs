//! XML-only wrapper around a [`XmlFragment`].
//!
//! Some generated types are binary-serialized *and* deserialized from XML. Their XML fallback
//! values have no binary representation (an untyped payload has no length prefix), but the binary
//! derive still requires every field type to implement the binary traits.
//!
//! `XmlOnlyFragment` implements those traits as an explicit placeholder: reading yields an empty
//! fragment (the format never produces one) and writing fails. The serde implementation delegates
//! to the wrapped fragment, so XML values pass through unchanged.
//!
//! This wrapper exists in this crate rather than in `xml-fragment` so that the published
//! `xml-fragment` crate does not depend on the binary serialization framework.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use xml_fragment::XmlFragment;

/// A fragment that can only travel through XML.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct XmlOnlyFragment(pub XmlFragment);

impl From<XmlFragment> for XmlOnlyFragment {
    fn from(value: XmlFragment) -> Self {
        Self(value)
    }
}

impl Serialize for XmlOnlyFragment {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for XmlOnlyFragment {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        XmlFragment::deserialize(deserializer).map(Self)
    }
}

impl<'a> deku::DekuReader<'a, ()> for XmlOnlyFragment {
    fn from_reader_with_ctx<R: deku::no_std_io::Read + deku::no_std_io::Seek>(
        _reader: &mut deku::reader::Reader<R>,
        _: (),
    ) -> Result<Self, deku::DekuError> {
        // The binary format never produces an XML-only value; this only satisfies the derive.
        Ok(Self::default())
    }
}

impl deku::DekuWriter<()> for XmlOnlyFragment {
    fn to_writer<W: deku::no_std_io::Write + deku::no_std_io::Seek>(
        &self,
        _writer: &mut deku::writer::Writer<W>,
        _: (),
    ) -> Result<(), deku::DekuError> {
        Err(deku::DekuError::Parse(
            "cannot encode an XML-only fallback into the binary format".into(),
        ))
    }
}
