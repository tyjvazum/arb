use {
    super::*,
    crate::off_chain,
    bitcoin::{
        blockdata::{
            opcodes,
            script::{
                self,
                Instruction,
                Instructions,
            },
        },
        hashes::{
            hex::ToHex,
            sha256,
            Hash,
        },
        util::taproot::TAPROOT_ANNEX_PREFIX,
        Script,
        Witness,
    },
    brotli::{
        CompressorWriter,
        Decompressor,
    },
    include_dir::{
        include_dir,
        Dir,
    },
    std::{
        ffi::OsStr,
        io::{
            Cursor,
            Read,
            Write,
        },
        iter::Peekable,
        str,
    },
    version_compare::Version,
};

struct LimitedReader<R> {
    inner: io::BufReader<R>,
    limit: usize,
    total_read: usize,
}

impl<R: Read> LimitedReader<R> {
    fn new(
        inner: R,
        limit: usize,
    ) -> Self {
        Self {
            inner: io::BufReader::new(inner),
            limit,
            total_read: 0,
        }
    }
}

impl<R: Read> Read for LimitedReader<R> {
    fn read(
        &mut self,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        if self.total_read >= self.limit {
            return Ok(0);
        }
        let remaining = self.limit - self.total_read;
        let to_read = buf.len().min(remaining);
        let n = self.inner.read(&mut buf[..to_read])?;
        self.total_read += n;
        Ok(n)
    }
}

const ORDV1_GENERAL_MESSAGE: &str = "This inscription is using the ordv1 protocol. If \
you see this message, you're likely using an outdated ordv0-only client or explorer. Consider \
upgrading to the software referenced in this message, asking your current software provider to add \
support for ordv1, or switching to other software compatible with ordv1.";
const ORDV1_COMPRESSED_MESSAGE: &str = "This inscription is compressed using the ordv1 protocol. \
If you see this message, you're likely using an outdated ordv0-only client or explorer. Consider \
upgrading to the software referenced in this message, asking your current software provider to add \
support for ordv1, or switching to other software compatible with ordv1.";
const ORDV1_OFF_CHAIN_MESSAGE: &str = "This inscription's content is off-chain as a torrent using \
the ordv1 protocol. If you see this message, you're likely using an outdated ordv0-only client or \
explorer. Consider upgrading to the software referenced in this message, asking your current \
software provider to add support for ordv1, or switching to other software compatible with ordv1.";

const ORDV1_SOFTWARE_MESSAGE: &str = "https://github.com/tyjvazum/arb";

const BODY_TAG: &[u8] = &[];
const CONTENT_TYPE_TAG: &[u8] = &[1];

#[derive(Deserialize, Serialize)]
pub struct Expansion {
    protocol: String,
    protocol_version: String,
    protocol_properties: String,
    compression: Option<String>,
    offchain: Option<String>,
    content: Option<String>,
    content_hash: Option<String>,
    content_type: Option<String>,
    content_metadata: Option<String>,
    wrapped: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Inscription {
    content_type: Option<Vec<u8>>,
    body: Option<Vec<u8>>,
    tracking: bool,
    content_metadata: Option<Vec<u8>>,
    protocol_properties: Option<String>,
}

impl Inscription {
    #[cfg(test)]
    pub(crate) fn new(
        content_type: Option<Vec<u8>>,
        body: Option<Vec<u8>>,
    ) -> Self {
        Self {
            content_type,
            body,
            tracking: true,
            content_metadata: None,
            protocol_properties: None,
        }
    }

    pub(crate) fn from_transaction(tx: &Transaction) -> Option<Inscription> {
        InscriptionParser::parse(&tx.input.get(0)?.witness).ok()
    }

    pub(crate) fn from_file(
        chain: Chain,
        path: impl AsRef<Path>,
        title: Option<String>,
        subtitle: Option<String>,
        compression: bool,
        offchain: bool,
        torrent_path: Option<impl AsRef<Path>>,
        tracker_url: &str,
        peer_addr: &str,
        metadata_path: Option<impl AsRef<Path>>,
        properties_path: Option<impl AsRef<Path>>,
        license: Option<String>,
        protocol_id: String,
        description: Option<String>,
    ) -> Result<Self, Error> {
        if compression && offchain {
            bail!("Compression and offchain must not be enabled at the same time!");
        }

        let path = path.as_ref();

        let body =
            fs::read(path).with_context(|| format!("io error reading {}", path.display()))?;

        let mut compressed = Vec::new();

        if compression || metadata_path.is_some() {
            let mut compressor = CompressorWriter::new(&mut compressed, 4096, 11, 22);
            compressor.write_all(&body)?;
            drop(compressor);
        }

        let (result, content_encoding) = if (compression || metadata_path.is_some())
            && (1.0 - (compressed.len() as f64 / body.len() as f64)) > 0.0
        {
            (compressed, Some(true))
        } else {
            (body, None)
        };

        if let Some(limit) = chain.inscription_content_size_limit() {
            let len = result.len();
            if len > limit {
                bail!(
                    "Content size of {len} bytes exceeds {limit} byte limit for {chain} \
                inscriptions!"
                );
            }
        }

        let metadata_json = match &metadata_path {
            Some(p) if p.as_ref().extension().and_then(OsStr::to_str) == Some("json") => {
                let contents = fs::read_to_string(p).expect("Unable to read metadata from file!");
                let result: serde_json::Value = serde_json::from_str(&contents).unwrap();
                Some(result.to_string())
            },
            _ => None,
        };

        let (encoded_metadata, metadata_bytes) = if let Some(md) = metadata_json {
            let bytes = md.clone().into_bytes();
            (Some(base64::encode(&md)), Some(bytes))
        } else {
            (None, None)
        };

        // Begin protocol-level properties file handling.
        static PROTOCOLS_DIR: Dir<'_> = include_dir!("protocols/");

        // Default protocols are version locked.
        let spec_name = if protocol_id == *"ord-v1" {
            "ord-v1.0.0.json".to_string()
        } else if protocol_id == *"pub-v1" || protocol_id == *"pub" {
            "pub-v1.0.0.json".to_string()
        } else {
            // Get list of spec filenames from protocols folder.
            let files = PROTOCOLS_DIR.files();
            let mut active_spec = "".to_string();
            for file in files {
                // Check if any begin with protocol_id.
                let path_string = file.path().display().to_string();
                if file.path().starts_with(&protocol_id) {
                    if active_spec != *"" {
                        // If multiple, choose the one with the highest SemVer.
                        if Version::from(path_string.as_str()).unwrap()
                            > Version::from(active_spec.as_str()).unwrap()
                        {
                            active_spec = path_string.clone();
                        }
                    } else {
                        active_spec = path_string.clone();
                    }
                }
            }
            // Return spec or ""
            active_spec
        };

        let protocol_spec = if spec_name != *"" {
            let r = PROTOCOLS_DIR
                .get_file(spec_name)
                .unwrap()
                .contents_utf8()
                .unwrap()
                .replace('\n', "");
            r
        } else {
            "{}".to_string()
        };

        let mut protocol_json: serde_json::Value =
            serde_json::from_str(&protocol_spec).expect("Unable to parse properties JSON!");
        // End protocol-level properties file handling.

        // Special handling for 'ord' in CLI API w/o directly using a prop file.
        // These will get replaced if a prop file is provided.
        if protocol_id == *"ord-v1" {
            protocol_json["title"] = if let Some(value) = title {
                value.into()
            } else {
                "".into()
            };

            protocol_json["subtitle"] = if let Some(value) = subtitle {
                value.into()
            } else {
                "".into()
            };

            protocol_json["license"] = if let Some(value) = license {
                value.into()
            } else {
                "".into()
            };

            protocol_json["description"] = if let Some(value) = description {
                value.into()
            } else {
                "".into()
            };

            if content_encoding.is_some() {
                protocol_json["comment"] = ORDV1_COMPRESSED_MESSAGE.to_owned().into();
            } else if offchain {
                protocol_json["comment"] = ORDV1_OFF_CHAIN_MESSAGE.to_owned().into();
            } else {
                protocol_json["comment"] = ORDV1_GENERAL_MESSAGE.to_owned().into();
            }

            protocol_json["description"] = ORDV1_SOFTWARE_MESSAGE.to_owned().into();
        } else {
            protocol_json["title"] = "".into();
            protocol_json["subtitle"] = "".into();
            protocol_json["license"] = "".into();
        }

        let tracking = protocol_json["tracking"] == true || protocol_id == *"ord-v0";

        // Begin inscription-level properties file handling.
        let properties_json = match &properties_path {
            Some(p) if p.as_ref().extension().and_then(OsStr::to_str) == Some("json") => {
                let contents = fs::read_to_string(p).expect("Unable to read props from file!");
                let result: Option<serde_json::Value> =
                    Some(serde_json::from_str(&contents).unwrap());
                result
            },
            _ => None,
        };

        if let Some(properties) = properties_json {
            for pair in properties.as_object().unwrap() {
                let (key, value) = pair;
                if let Some(_field) = protocol_json.get("key") {
                    protocol_json[key] = value.clone();
                }
            }
        }
        // End inscription-level properties file handling.

        if content_encoding.is_some() {
            let compressed = Expansion {
                protocol: protocol_id,
                protocol_version: protocol_json["version"].to_string(),
                protocol_properties: protocol_json.to_string(),
                compression: Some("br base64".to_owned()),
                offchain: None,
                content: Some(base64::encode(&result)),
                content_hash: Some(sha256::Hash::hash(&result).into_inner().to_vec().to_hex()),
                content_type: Some(Media::content_type_for_path(path)?.to_owned()),
                content_metadata: encoded_metadata,
                wrapped: true,
            };

            let json = serde_json::to_string(&compressed)?;

            Ok(Self {
                content_type: Some("application/json".as_bytes().to_vec()),
                body: Some(json.into()),
                tracking,
                content_metadata: metadata_bytes,
                protocol_properties: Some(protocol_json.to_string()),
            })
        } else if offchain {
            let magnet_and_sha256hash =
                off_chain::make_offchain_inscription(path, torrent_path, tracker_url, peer_addr)?;

            let offchain = Expansion {
                protocol: protocol_id,
                protocol_version: protocol_json["version"].to_string(),
                protocol_properties: protocol_json.to_string(),
                compression: None,
                offchain: Some(magnet_and_sha256hash[0].clone()),
                content: None,
                content_hash: Some(magnet_and_sha256hash[1].clone()),
                content_type: Some(Media::content_type_for_path(path)?.to_owned()),
                content_metadata: encoded_metadata,
                wrapped: true,
            };

            let json = serde_json::to_string(&offchain)?;

            Ok(Self {
                content_type: Some("application/json".as_bytes().to_vec()),
                body: Some(json.into()),
                tracking,
                content_metadata: metadata_bytes,
                protocol_properties: Some(protocol_json.to_string()),
            })
        } else if protocol_id != *"ord-v0" {
            let v1wrapper = Expansion {
                protocol: protocol_id,
                protocol_version: protocol_json["version"].to_string(),
                protocol_properties: protocol_json.to_string(),
                compression: None,
                offchain: None,
                content: Some(base64::encode(&result)),
                content_hash: Some(sha256::Hash::hash(&result).into_inner().to_vec().to_hex()),
                content_type: Some(Media::content_type_for_path(path)?.to_owned()),
                content_metadata: encoded_metadata,
                wrapped: true,
            };

            let json = serde_json::to_string(&v1wrapper)?;

            Ok(Self {
                content_type: Some("application/json".as_bytes().to_vec()),
                body: Some(json.into()),
                tracking,
                content_metadata: metadata_bytes,
                protocol_properties: Some(protocol_json.to_string()),
            })
        } else {
            let content_type = Media::content_type_for_path(path)?;

            Ok(Self {
                content_type: Some(content_type.into()),
                body: Some(result),
                tracking: true,
                content_metadata: None,
                protocol_properties: None,
            })
        }
    }

    fn append_reveal_script_to_builder(
        &self,
        mut builder: script::Builder,
    ) -> script::Builder {
        let protocol: &[u8] = if self.tracking { b"ord" } else { b"pub" };

        builder = builder
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(protocol);

        if let Some(content_type) = &self.content_type {
            builder = builder
                .push_slice(CONTENT_TYPE_TAG)
                .push_slice(content_type);
        }

        if let Some(body) = &self.body {
            builder = builder.push_slice(BODY_TAG);
            for chunk in body.chunks(520) {
                builder = builder.push_slice(chunk);
            }
        }

        builder.push_opcode(opcodes::all::OP_ENDIF)
    }

    pub(crate) fn append_reveal_script(
        &self,
        builder: script::Builder,
    ) -> Script {
        self.append_reveal_script_to_builder(builder).into_script()
    }

    pub(crate) fn media(&self) -> Media {
        if self.body.is_none() {
            return Media::Unknown;
        }

        let Some(content_type) = self.content_type() else {
      return Media::Unknown;
    };

        content_type.parse().unwrap_or(Media::Unknown)
    }

    pub(crate) fn body(&self) -> Option<&[u8]> {
        Some(self.body.as_ref()?)
    }

    pub(crate) fn into_body_metadata_and_props(
        self
    ) -> (Option<Vec<u8>>, Option<Vec<u8>>, Option<String>) {
        (self.body, self.content_metadata, self.protocol_properties)
    }

    pub(crate) fn content_length(&self) -> Option<usize> {
        Some(self.body()?.len())
    }

    pub(crate) fn content_type(&self) -> Option<&str> {
        str::from_utf8(self.content_type.as_ref()?).ok()
    }

    #[cfg(test)]
    pub(crate) fn to_witness(&self) -> Witness {
        let builder = script::Builder::new();

        let script = self.append_reveal_script(builder);

        let mut witness = Witness::new();

        witness.push(script);
        witness.push([]);

        witness
    }
}

#[derive(Debug, PartialEq)]
enum InscriptionError {
    EmptyWitness,
    InvalidInscription,
    KeyPathSpend,
    NoInscription,
    Script(script::Error),
    UnrecognizedEvenField,
}

type Result<T, E = InscriptionError> = std::result::Result<T, E>;

struct InscriptionParser<'a> {
    instructions: Peekable<Instructions<'a>>,
}

impl<'a> InscriptionParser<'a> {
    fn parse(witness: &Witness) -> Result<Inscription> {
        if witness.is_empty() {
            return Err(InscriptionError::EmptyWitness);
        }

        if witness.len() == 1 {
            return Err(InscriptionError::KeyPathSpend);
        }

        let annex = witness
            .last()
            .and_then(|element| element.first().map(|byte| *byte == TAPROOT_ANNEX_PREFIX))
            .unwrap_or(false);

        if witness.len() == 2 && annex {
            return Err(InscriptionError::KeyPathSpend);
        }

        let script = witness
            .iter()
            .nth(if annex {
                witness.len() - 1
            } else {
                witness.len() - 2
            })
            .unwrap();

        InscriptionParser {
            instructions: Script::from(Vec::from(script)).instructions().peekable(),
        }
        .parse_script()
    }

    fn parse_script(mut self) -> Result<Inscription> {
        loop {
            let next = self.advance()?;

            if next == Instruction::PushBytes(&[]) {
                if let Some(inscription) = self.parse_inscription()? {
                    return Ok(inscription);
                }
            }
        }
    }

    fn advance(&mut self) -> Result<Instruction<'a>> {
        self.instructions
            .next()
            .ok_or(InscriptionError::NoInscription)?
            .map_err(InscriptionError::Script)
    }

    fn parse_inscription(&mut self) -> Result<Option<Inscription>> {
        if self.advance()? == Instruction::Op(opcodes::all::OP_IF) {
            if !self.accept(Instruction::PushBytes(b"ord"))? {
                return Err(InscriptionError::NoInscription);
            }

            let mut fields = BTreeMap::new();

            loop {
                match self.advance()? {
                    Instruction::PushBytes(BODY_TAG) => {
                        let mut body = Vec::new();
                        while !self.accept(Instruction::Op(opcodes::all::OP_ENDIF))? {
                            body.extend_from_slice(self.expect_push()?);
                        }
                        fields.insert(BODY_TAG, body);
                        break;
                    },
                    Instruction::PushBytes(tag) => {
                        if fields.contains_key(tag) {
                            return Err(InscriptionError::InvalidInscription);
                        }
                        fields.insert(tag, self.expect_push()?.to_vec());
                    },
                    Instruction::Op(opcodes::all::OP_ENDIF) => break,
                    _ => return Err(InscriptionError::InvalidInscription),
                }
            }

            let content_type = fields.remove(CONTENT_TYPE_TAG);
            let mut body = fields.remove(BODY_TAG);

            for tag in fields.keys() {
                if let Some(lsb) = tag.first() {
                    if lsb % 2 == 0 {
                        return Err(InscriptionError::UnrecognizedEvenField);
                    }
                }
            }

            if content_type.is_some()
                && content_type.clone().unwrap() == "application/json".as_bytes().to_vec()
                && body.is_some()
            {
                if let Ok(_utf8) = str::from_utf8(body.clone().unwrap().as_slice()) {
                    let expansion: Expansion = serde_json::from_str(
                        str::from_utf8(body.clone().unwrap().as_slice()).unwrap(),
                    )
                    .unwrap_or_else(|_| Expansion {
                        protocol: "error".to_owned(),
                        protocol_version: "error".to_owned(),
                        protocol_properties: "".to_owned(),
                        compression: None,
                        offchain: None,
                        content: None,
                        content_hash: None,
                        content_type: None,
                        content_metadata: None,
                        wrapped: false,
                    });

                    if expansion.wrapped {
                        let content_metadata = if expansion.content_metadata.is_some() {
                            Some(expansion.content_metadata.unwrap().into_bytes())
                        } else {
                            None
                        };

                        let (protocol_properties, tracked) =
                            if expansion.protocol_properties != *"{}" {
                                let protocol_protocol_json: serde_json::Value =
                                    serde_json::from_str(&expansion.protocol_properties)
                                        .expect("Unable to parse protocol properties!");

                                (
                                    Some(expansion.protocol_properties),
                                    protocol_protocol_json["tracking"] == true,
                                )
                            } else {
                                (None, false)
                            };

                        if expansion.compression.is_some() {
                            let content = base64::decode(expansion.content.unwrap()).unwrap();

                            // Limit input data size to 10MB max to prevent DoS vector.
                            let max_input_size = 10000000;
                            let input_cursor = Cursor::new(&content);
                            let input_limited = input_cursor.take(max_input_size);
                            #[allow(clippy::cast_possible_truncation)]
                            let mut limited_reader =
                                LimitedReader::new(input_limited, max_input_size as usize);
                            let mut decompressor = Decompressor::new(&mut limited_reader, 4096);
                            let mut decompressed = Vec::new();

                            match decompressor.read_to_end(&mut decompressed) {
                                Ok(_) => (),
                                Err(e) => {
                                    if e.kind() == std::io::ErrorKind::InvalidData {
                                        println!("Decompression failed due to invalid data!");
                                    } else {
                                        println!("Decompression failed due to an error: {}", e);
                                    }
                                },
                            }
                            body = Some(decompressed);
                        }

                        return Ok(Some(Inscription {
                            content_type,
                            body,
                            tracking: tracked,
                            content_metadata,
                            protocol_properties,
                        }));
                    }
                } else {
                    return Ok(Some(Inscription {
                        content_type,
                        body,
                        tracking: true,
                        content_metadata: None,
                        protocol_properties: None,
                    }));
                }
            } else {
                return Ok(Some(Inscription {
                    content_type,
                    body,
                    tracking: true,
                    content_metadata: None,
                    protocol_properties: None,
                }));
            }
        }

        Ok(None)
    }

    fn expect_push(&mut self) -> Result<&'a [u8]> {
        match self.advance()? {
            Instruction::PushBytes(bytes) => Ok(bytes),
            _ => Err(InscriptionError::InvalidInscription),
        }
    }

    fn accept(
        &mut self,
        instruction: Instruction,
    ) -> Result<bool> {
        match self.instructions.peek() {
            Some(Ok(next)) => {
                if *next == instruction {
                    self.advance()?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
            Some(Err(err)) => Err(InscriptionError::Script(*err)),
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(payload: &[&[u8]]) -> Witness {
        let mut builder = script::Builder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF);

        for data in payload {
            builder = builder.push_slice(data);
        }

        let script = builder.push_opcode(opcodes::all::OP_ENDIF).into_script();

        Witness::from_vec(vec![script.into_bytes(), Vec::new()])
    }

    #[test]
    fn empty() {
        assert_eq!(
            InscriptionParser::parse(&Witness::new()),
            Err(InscriptionError::EmptyWitness)
        );
    }

    #[test]
    fn ignore_key_path_spends() {
        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![Vec::new()])),
            Err(InscriptionError::KeyPathSpend),
        );
    }

    #[test]
    fn ignore_key_path_spends_with_annex() {
        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![Vec::new(), vec![0x50]])),
            Err(InscriptionError::KeyPathSpend),
        );
    }

    #[test]
    fn ignore_unparsable_scripts() {
        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![vec![0x01], Vec::new()])),
            Err(InscriptionError::Script(script::Error::EarlyEndOfScript)),
        );
    }

    #[test]
    fn no_inscription() {
        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![
                Script::new().into_bytes(),
                Vec::new()
            ])),
            Err(InscriptionError::NoInscription),
        );
    }

    #[test]
    fn duplicate_field() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                b"ord",
            ])),
            Err(InscriptionError::InvalidInscription),
        );
    }

    #[test]
    fn valid() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                b"ord",
            ])),
            Ok(inscription("text/plain;charset=utf-8", "ord")),
        );
    }

    #[test]
    fn valid_with_unknown_tag() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[3],
                b"bar",
                &[],
                b"ord",
            ])),
            Ok(inscription("text/plain;charset=utf-8", "ord")),
        );
    }

    #[test]
    fn no_content_tag() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[b"ord", &[1], b"text/plain;charset=utf-8"])),
            Ok(Inscription {
                content_type: Some(b"text/plain;charset=utf-8".to_vec()),
                body: None,
                tracking: true,
                content_metadata: None,
                protocol_properties: None,
            }),
        );
    }

    #[test]
    fn no_content_type() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[b"ord", &[], b"foo"])),
            Ok(Inscription {
                content_type: None,
                body: Some(b"foo".to_vec()),
                tracking: true,
                content_metadata: None,
                protocol_properties: None,
            }),
        );
    }

    #[test]
    fn valid_body_in_multiple_pushes() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                b"foo",
                b"bar"
            ])),
            Ok(inscription("text/plain;charset=utf-8", "foobar")),
        );
    }

    #[test]
    fn valid_body_in_zero_pushes() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[b"ord", &[1], b"text/plain;charset=utf-8", &[]])),
            Ok(inscription("text/plain;charset=utf-8", "")),
        );
    }

    #[test]
    fn valid_body_in_multiple_empty_pushes() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
            ])),
            Ok(inscription("text/plain;charset=utf-8", "")),
        );
    }

    #[test]
    fn valid_ignore_trailing() {
        let script = script::Builder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice(&[1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice(&[])
            .push_slice(b"ord")
            .push_opcode(opcodes::all::OP_ENDIF)
            .push_opcode(opcodes::all::OP_CHECKSIG)
            .into_script();

        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![script.into_bytes(), Vec::new()])),
            Ok(inscription("text/plain;charset=utf-8", "ord")),
        );
    }

    #[test]
    fn valid_ignore_preceding() {
        let script = script::Builder::new()
            .push_opcode(opcodes::all::OP_CHECKSIG)
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice(&[1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice(&[])
            .push_slice(b"ord")
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![script.into_bytes(), Vec::new()])),
            Ok(inscription("text/plain;charset=utf-8", "ord")),
        );
    }

    #[test]
    fn valid_ignore_inscriptions_after_first() {
        let script = script::Builder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice(&[1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice(&[])
            .push_slice(b"foo")
            .push_opcode(opcodes::all::OP_ENDIF)
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice(b"ord")
            .push_slice(&[1])
            .push_slice(b"text/plain;charset=utf-8")
            .push_slice(&[])
            .push_slice(b"bar")
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![script.into_bytes(), Vec::new()])),
            Ok(inscription("text/plain;charset=utf-8", "foo")),
        );
    }

    #[test]
    fn invalid_utf8_does_not_render_inscription_invalid() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[
                b"ord",
                &[1],
                b"text/plain;charset=utf-8",
                &[],
                &[0b10000000]
            ])),
            Ok(inscription("text/plain;charset=utf-8", [0b10000000])),
        );
    }

    #[test]
    fn no_endif() {
        let script = script::Builder::new()
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF)
            .push_slice("ord".as_bytes())
            .into_script();

        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![script.into_bytes(), Vec::new()])),
            Err(InscriptionError::NoInscription)
        );
    }

    #[test]
    fn no_op_false() {
        let script = script::Builder::new()
            .push_opcode(opcodes::all::OP_IF)
            .push_slice("ord".as_bytes())
            .push_opcode(opcodes::all::OP_ENDIF)
            .into_script();

        assert_eq!(
            InscriptionParser::parse(&Witness::from_vec(vec![script.into_bytes(), Vec::new()])),
            Err(InscriptionError::NoInscription)
        );
    }

    #[test]
    fn empty_envelope() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[])),
            Err(InscriptionError::NoInscription)
        );
    }

    #[test]
    fn wrong_magic_number() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[b"foo"])),
            Err(InscriptionError::NoInscription),
        );
    }

    #[test]
    fn extract_from_transaction() {
        let tx = Transaction {
            version: 0,
            lock_time: bitcoin::PackedLockTime(0),
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: Script::new(),
                sequence: Sequence(0),
                witness: envelope(&[b"ord", &[1], b"text/plain;charset=utf-8", &[], b"ord"]),
            }],
            output: Vec::new(),
        };

        assert_eq!(
            Inscription::from_transaction(&tx),
            Some(inscription("text/plain;charset=utf-8", "ord")),
        );
    }

    #[test]
    fn do_not_extract_from_second_input() {
        let tx = Transaction {
            version: 0,
            lock_time: bitcoin::PackedLockTime(0),
            input: vec![
                TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: Script::new(),
                    sequence: Sequence(0),
                    witness: Witness::new(),
                },
                TxIn {
                    previous_output: OutPoint::null(),
                    script_sig: Script::new(),
                    sequence: Sequence(0),
                    witness: inscription("foo", [1; 1040]).to_witness(),
                },
            ],
            output: Vec::new(),
        };

        assert_eq!(Inscription::from_transaction(&tx), None);
    }

    #[test]
    fn do_not_extract_from_second_envelope() {
        let mut builder = script::Builder::new();
        builder = inscription("foo", [1; 100]).append_reveal_script_to_builder(builder);
        builder = inscription("bar", [1; 100]).append_reveal_script_to_builder(builder);

        let witness = Witness::from_vec(vec![builder.into_script().into_bytes(), Vec::new()]);

        let tx = Transaction {
            version: 0,
            lock_time: bitcoin::PackedLockTime(0),
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: Script::new(),
                sequence: Sequence(0),
                witness,
            }],
            output: Vec::new(),
        };

        assert_eq!(
            Inscription::from_transaction(&tx),
            Some(inscription("foo", [1; 100]))
        );
    }

    #[test]
    fn inscribe_png() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[b"ord", &[1], b"image/png", &[], &[1; 100]])),
            Ok(inscription("image/png", [1; 100])),
        );
    }

    #[test]
    fn reveal_script_chunks_data() {
        assert_eq!(
            inscription("foo", [])
                .append_reveal_script(script::Builder::new())
                .instructions()
                .count(),
            7
        );

        assert_eq!(
            inscription("foo", [0; 1])
                .append_reveal_script(script::Builder::new())
                .instructions()
                .count(),
            8
        );

        assert_eq!(
            inscription("foo", [0; 520])
                .append_reveal_script(script::Builder::new())
                .instructions()
                .count(),
            8
        );

        assert_eq!(
            inscription("foo", [0; 521])
                .append_reveal_script(script::Builder::new())
                .instructions()
                .count(),
            9
        );

        assert_eq!(
            inscription("foo", [0; 1040])
                .append_reveal_script(script::Builder::new())
                .instructions()
                .count(),
            9
        );

        assert_eq!(
            inscription("foo", [0; 1041])
                .append_reveal_script(script::Builder::new())
                .instructions()
                .count(),
            10
        );
    }

    #[test]
    fn chunked_data_is_parsable() {
        let mut witness = Witness::new();

        witness.push(&inscription("foo", [1; 1040]).append_reveal_script(script::Builder::new()));

        witness.push([]);

        assert_eq!(
            InscriptionParser::parse(&witness).unwrap(),
            inscription("foo", [1; 1040]),
        );
    }

    #[test]
    fn round_trip_with_no_fields() {
        let mut witness = Witness::new();

        witness.push(
            &Inscription {
                content_type: None,
                body: None,
                tracking: true,
                content_metadata: None,
                protocol_properties: None,
            }
            .append_reveal_script(script::Builder::new()),
        );

        witness.push([]);

        assert_eq!(
            InscriptionParser::parse(&witness).unwrap(),
            Inscription {
                content_type: None,
                body: None,
                tracking: true,
                content_metadata: None,
                protocol_properties: None,
            }
        );
    }

    #[test]
    fn unknown_odd_fields_are_ignored() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[b"ord", &[3], &[0]])),
            Ok(Inscription {
                content_type: None,
                body: None,
                tracking: true,
                content_metadata: None,
                protocol_properties: None,
            }),
        );
    }

    #[test]
    fn unknown_even_fields_are_invalid() {
        assert_eq!(
            InscriptionParser::parse(&envelope(&[b"ord", &[2], &[0]])),
            Err(InscriptionError::UnrecognizedEvenField),
        );
    }
}
