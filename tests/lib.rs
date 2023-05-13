#![allow(clippy::type_complexity, clippy::manual_map)]

use {
    self::{
        command_builder::CommandBuilder,
        expected::Expected,
        test_server::TestServer,
    },
    bip39::Mnemonic,
    bitcoin::{
        blockdata::constants::COIN_VALUE,
        Network,
        OutPoint,
        Txid,
    },
    executable_path::executable_path,
    include_dir::{
        include_dir,
        Dir,
    },
    pretty_assertions::assert_eq as pretty_assert_eq,
    regex::Regex,
    reqwest::{
        StatusCode,
        Url,
    },
    serde::{
        de::DeserializeOwned,
        Deserialize,
    },
    std::{
        fs,
        net::TcpListener,
        path::Path,
        process::{
            Child,
            Command,
            Stdio,
        },
        str::{
            self,
            FromStr,
        },
        thread,
        time::Duration,
    },
    tempfile::TempDir,
    test_bitcoincore_rpc::Sent,
};

macro_rules! assert_regex_match {
    ($string:expr, $pattern:expr $(,)?) => {
        let regex = Regex::new(&format!("^(?s){}$", $pattern)).unwrap();
        let string = $string;

        if !regex.is_match(string.as_ref()) {
            panic!(
                "Regex:\n\n{}\n\n…did not match string:\n\n{}",
                regex, string
            );
        }
    };
}

#[derive(Deserialize, Debug)]
struct Inscribe {
    #[allow(dead_code)]
    commit: Txid,
    inscription: String,
    reveal: Txid,
    fees: u64,
}

static INSCRIPTION_DIR: Dir<'_> = include_dir!("tests/inscriptions");

fn inscribe(
    rpc_server: &test_bitcoincore_rpc::Handle,
    filename: &str,
    compression: bool,
    metadata_filename: Option<&str>,
) -> Inscribe {
    rpc_server.mine_blocks(1);

    let content = INSCRIPTION_DIR
        .get_file(filename)
        .unwrap()
        .contents_utf8()
        .unwrap()
        .replace('\n', "");

    let metadata: Option<String> = if let Some(value) = metadata_filename {
        Some(
            INSCRIPTION_DIR
                .get_file(value)
                .unwrap()
                .contents_utf8()
                .unwrap()
                .replace('\n', ""),
        )
    } else {
        None
    };

    let command = if compression {
        if metadata_filename.is_some() {
            format!(
                "wallet inscribe --protocol-id ord-v1 --metadata-file {metadata} --fee-rate 1 --compression {file}",
                metadata = metadata_filename.unwrap(),
                file = filename
            )
        } else {
            format!(
                "wallet inscribe --protocol-id ord-v1 --fee-rate 1 --compression {}",
                filename
            )
        }
    } else if metadata_filename.is_some() {
        format!(
            "wallet inscribe --protocol-id ord-v1 --metadata-file {metadata} --fee-rate 1 --compression {file}",
            metadata = metadata_filename.unwrap(),
            file = filename
        )
    } else {
        format!("wallet inscribe --fee-rate 1 {}", filename)
    };

    let output = if let Some(value) = metadata {
        CommandBuilder::new(command)
            .write(filename, content)
            .write(metadata_filename.unwrap(), value.as_str())
            .rpc_server(rpc_server)
            .output()
    } else {
        CommandBuilder::new(command)
            .write(filename, content)
            .rpc_server(rpc_server)
            .output()
    };

    rpc_server.mine_blocks(1);

    output
}

#[derive(Deserialize)]
struct Create {
    mnemonic: Mnemonic,
}

fn create_wallet(rpc_server: &test_bitcoincore_rpc::Handle) {
    CommandBuilder::new(format!("--chain {} wallet create", rpc_server.network()))
        .rpc_server(rpc_server)
        .output::<Create>();
}

mod command_builder;
mod core;
mod epochs;
mod expected;
mod find;
mod index;
mod info;
mod list;
mod parse;
mod server;
mod subsidy;
mod supply;
mod test_server;
mod traits;
mod version;
mod wallet;
