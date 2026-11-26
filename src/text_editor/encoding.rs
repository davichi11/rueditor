//! 磁盘编码与 BOM，编辑器内部始终使用 UTF-8。
use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use std::{borrow::Cow, io};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16LeBom,
    Utf16Be,
    Utf16BeBom,
    Legacy(&'static encoding_rs::Encoding),
}

impl Encoding {
    pub(super) const OPTIONS: &[Self] = &[
        Self::Utf8,
        Self::Utf8Bom,
        Self::Utf16LeBom,
        Self::Utf16BeBom,
        Self::Utf16Le,
        Self::Utf16Be,
        Self::Legacy(encoding_rs::GBK),
        Self::Legacy(encoding_rs::GB18030),
        Self::Legacy(encoding_rs::BIG5),
        Self::Legacy(encoding_rs::SHIFT_JIS),
        Self::Legacy(encoding_rs::EUC_JP),
        Self::Legacy(encoding_rs::EUC_KR),
        Self::Legacy(encoding_rs::WINDOWS_1252),
        Self::Legacy(encoding_rs::WINDOWS_1251),
        Self::Legacy(encoding_rs::WINDOWS_1250),
        Self::Legacy(encoding_rs::KOI8_R),
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16LeBom => "UTF-16 LE BOM",
            Self::Utf16Be => "UTF-16 BE",
            Self::Utf16BeBom => "UTF-16 BE BOM",
            Self::Legacy(codec) => codec.name(),
        }
    }

    fn codec(self) -> &'static encoding_rs::Encoding {
        match self {
            Self::Utf8 | Self::Utf8Bom => encoding_rs::UTF_8,
            Self::Utf16Le | Self::Utf16LeBom => encoding_rs::UTF_16LE,
            Self::Utf16Be | Self::Utf16BeBom => encoding_rs::UTF_16BE,
            Self::Legacy(codec) => codec,
        }
    }

    fn bom(self) -> &'static [u8] {
        match self {
            Self::Utf8Bom => b"\xef\xbb\xbf",
            Self::Utf16LeBom => b"\xff\xfe",
            Self::Utf16BeBom => b"\xfe\xff",
            _ => b"",
        }
    }

    fn without_bom(self) -> Self {
        match self {
            Self::Utf8Bom => Self::Utf8,
            Self::Utf16LeBom => Self::Utf16Le,
            Self::Utf16BeBom => Self::Utf16Be,
            _ => self,
        }
    }

    fn decode_body(self, bytes: &[u8]) -> Option<Cow<'_, str>> {
        self.codec()
            .decode_without_bom_handling_and_without_replacement(bytes.get(self.bom().len()..)?)
    }

    pub(super) fn decode(bytes: &[u8], requested: Option<Self>) -> io::Result<(Self, String)> {
        if requested.is_none()
            && (bytes.starts_with(b"\xff\xfe\0\0") || bytes.starts_with(b"\0\0\xfe\xff"))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "暂不支持 UTF-32",
            ));
        }
        let detected = [Self::Utf8Bom, Self::Utf16LeBom, Self::Utf16BeBom]
            .into_iter()
            .find(|encoding| bytes.starts_with(encoding.bom()));
        let encoding = match requested {
            Some(requested) => detected
                .filter(|detected| detected.without_bom() == requested.without_bom())
                .unwrap_or_else(|| requested.without_bom()),
            None => match detected {
                Some(encoding) => encoding,
                None => {
                    return Self::detect_without_bom(bytes)
                        .map(|(encoding, text)| (encoding, text.into_owned()))
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "无法识别文本编码，可使用「以编码打开」",
                            )
                        });
                }
            },
        };
        let text = encoding.decode_body(bytes).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("无法按 {} 读取文本", encoding.label()),
            )
        })?;
        Ok((encoding, text.into_owned()))
    }

    fn detect_without_bom(bytes: &[u8]) -> Option<(Self, Cow<'_, str>)> {
        if bytes.contains(&0) {
            let encoding = Self::detect_utf16(bytes)?;
            let text = encoding
                .decode_body(bytes)
                .filter(|text| looks_like_text(text))?;
            return Some((encoding, text));
        }

        let utf8 = std::str::from_utf8(bytes).ok();
        if let Some(text) = utf8.filter(|_| !bytes.contains(&0x1b)) {
            return Some((Self::Utf8, Cow::Borrowed(text)));
        }

        let mut detector = EncodingDetector::new(Iso2022JpDetection::Allow);
        detector.feed(bytes, true);
        let codec = detector.guess(None, Utf8Detection::Deny);
        if let Some(text) = utf8.filter(|_| codec != encoding_rs::ISO_2022_JP) {
            return Some((Self::Utf8, Cow::Borrowed(text)));
        }
        let encoding = Self::Legacy(codec);
        let text = encoding
            .decode_body(bytes)
            .filter(|text| looks_like_text(text))?;
        // The GBK decoder also accepts four-byte GB18030 sequences. Preserve
        // the wider encoding when the text cannot round-trip through GBK.
        let encoding = if codec == encoding_rs::GBK && encoding.encode(&text).is_err() {
            Self::Legacy(encoding_rs::GB18030)
        } else {
            encoding
        };
        Some((encoding, text))
    }

    fn detect_utf16(bytes: &[u8]) -> Option<Self> {
        let pairs = bytes.chunks_exact(2);
        if !pairs.remainder().is_empty() {
            return None;
        }
        let (mut even_zeros, mut odd_zeros) = (0, 0);
        for pair in pairs {
            even_zeros += usize::from(pair[0] == 0);
            odd_zeros += usize::from(pair[1] == 0);
        }
        // BOM-less UTF-16 with ASCII has repeated zero bytes on one side
        // of the code units. Require repeated, strongly asymmetric evidence.
        if odd_zeros >= 2 && odd_zeros / 4 >= even_zeros {
            Some(Self::Utf16Le)
        } else if even_zeros >= 2 && even_zeros / 4 >= odd_zeros {
            Some(Self::Utf16Be)
        } else {
            None
        }
    }

    pub(super) fn encode(self, text: &str) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::from(self.bom());
        match self {
            Self::Utf8 | Self::Utf8Bom => bytes.extend_from_slice(text.as_bytes()),
            Self::Utf16Le | Self::Utf16LeBom => {
                bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            }
            Self::Utf16Be | Self::Utf16BeBom => {
                bytes.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
            }
            Self::Legacy(codec) => {
                let (encoded, _, had_errors) = codec.encode(text);
                if had_errors
                    || codec
                        .decode_without_bom_handling_and_without_replacement(&encoded)
                        .as_deref()
                        != Some(text)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} 无法无损表示文档中的字符", self.label()),
                    ));
                }
                bytes.extend_from_slice(&encoded);
            }
        }
        Ok(bytes)
    }
}

fn looks_like_text(text: &str) -> bool {
    text.chars()
        .all(|ch| !ch.is_control() || matches!(ch, '\t' | '\n' | '\r' | '\x0c' | '\u{0085}'))
}
