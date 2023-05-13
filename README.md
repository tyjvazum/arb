◉ `arb`
=====

`arb` is a command-line wallet, index, and explorer interface that
implements the `arb` protocol, which enables arbitrary protocols on top
of Bitcoin, such as Bitcoin NFTs/Ordinals &amp; Bitcoin Identities/Usernames.

It is experimental software, should be considered a work-in-progress, and has
no warranty. All features may not be fully implemented currently. See issues
and [LICENSE](LICENSE) for more details.

Join [the Gitter room](https://app.gitter.im/#/room/#arb-proto:gitter.im) to
chat about the `arb` ecosystem.

Features
------

- [ ] Arbitrary Protocols https://github.com/tyjvazum/arb/issues/1
  - [ ] Read Arbitrary Protocol Inscriptions
  - [x] Write Arbitrary Protocol Inscriptions

- [x] Content Compression https://github.com/tyjvazum/arb/issues/2

- [ ] Data Deduplication https://github.com/tyjvazum/arb/issues/3

- [ ] Hash-addressed Content https://github.com/tyjvazum/arb/issues/4

- [ ] Inscription Constraints https://github.com/tyjvazum/arb/issues/5

- [x] Inscription Metadata (JSON) https://github.com/tyjvazum/arb/issues/6

- [ ] Multipart Inscriptions https://github.com/tyjvazum/arb/issues/7

- [x] Non-tracked / Non-transferable Inscriptions https://github.com/tyjvazum/arb/issues/8

- [x] Off-chain Content (BitTorrent) https://github.com/tyjvazum/arb/issues/9

Default Protocols
------

- [ ] 📁 `bfs`: Bitcoin File System, enabling storage and retrieval of public files using a
  [filesystem](https://en.wikipedia.org/wiki/File_system) paradigm.

- [ ] 🪪 `bid`: Bitcoin Identifiers/Usernames, enabling unique, human-meaningful
  name registration natively on Bitcoin.

- [ ] ✨ `bnw`: Bitcoin NFT Walls, enabling a `bid` to showcase a curated
  collection of NFTs that it owns.

- [ ] 💎 `ord`: Bitcoin NFTs/Ordinals, enabling NFTs natively on Bitcoin by imbuing
  satoshis with numismatic value, allowing them to be collected and traded as
  curios.

- [x] ◉ `arb` supports arbitrary protocols on top of Bitcoin using inscriptions, so
additional protocols can be defined using a JSON specification file, which are
loaded to run the arbitrary protocol.

`bfs` Protocol
------

- Is associated with a specific `bid` Identifier/Username.

`bid` Protocol
------

- Characters can be alphanumeric with underscores, lowercase a through z,
  0 through 9, and _ in any combination.

- Length can be 1 through 16 characters, with 6 characters and shorter reserved
  for a future update, so 7 to 16 characters to start with.

- Usernames must be renewed periodically, likely every 52,500 blocks, which is
  about 1 year, but perhaps a shorter period initially to discourage speculation
  and encourage engagement.

- A "sunrise period" where a list of the top ten thousand domains are reserved,
  with the matching username claimable by publishing some specific data at a
  well-known location on the domain prior to the end of the sunrise period,
  which would be some specified block height.

`bnw` Protocol
------

- Is associated with a specific `bid` Identifier/Username.

- Is addressable at `USERNAME/WALL` where `USERNAME` is a valid `bid` inscription and
  `WALL` is the name for a `wal` inscription that the `bid` inscription is associated with.

- Has a text description that can be whatever the owner chooses.

`ord` Protocol
------

- Version 0 (ordv0): As defined in https://github.com/casey/ord.
  
- Version 1 (ordv1): Extended with new features, implemented through a backward-compatible,
  soft-fork mechanism termed Envelope Expansion.
    - Content Compression
    - Inscription Metadata (JSON)
    - Off-chain Content (BitTorrent)
    - Optional Title, Subtitle, Description, License, and Comment Fields
    - Upgradable Version Mechanism

Wallet
------

`arb` relies on Bitcoin Core for key management and transaction signing.
This has a number of implications that you must understand in order to use
`arb` wallet commands safely:

- Bitcoin Core is not aware of inscriptions and does not perform sat
  control. Using `bitcoin-cli` commands and RPC calls with `arb` wallets may
  lead to loss of inscriptions.

- `arb wallet` commands automatically load the `arb` wallet given by the
  `--wallet` option, which defaults to 'arb'. Keep in mind that after running
  an `arb wallet` command, an `arb` wallet may be loaded.

- Because `arb` has access to your Bitcoin Core wallets, `arb` should not be
  used with wallets that contain a material amount of funds. Keep ordinal and
  cardinal wallets segregated.

Installation
------------

`arb` is written in Rust and can be built from
[source](https://github.com/tyjvazum/arb). Pre-built binaries are available on the
[releases page](https://github.com/tyjvazum/arb/releases).

You can install the latest pre-built binary from the command line with:

```sh
curl --proto '=https' --tlsv1.2 -fsLS https://raw.githubusercontent.com/tyjvazum/arb/master/install.sh | bash -s
```

Once `arb` is installed, you should be able to run `arb --version` on the
command line.

Building
--------

On Debian and Ubuntu, `arb` requires `libssl-dev` when building from source:

```
sudo apt-get install libssl-dev
```

You'll also need Rust:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

To build `arb` from source:

```
git clone https://github.com/tyjvazum/arb.git
cd arb
cargo build --release
```

The default location for the `arb` binary once built is `./target/release/arb`.

`arb` requires `rustc` version 1.67.0 or later. Run `rustc --version` to ensure you have this 
version. Run `rustup update` to get the latest stable release.

Syncing
-------

`arb` requires a synced `bitcoind` node with `-txindex` to build the index of
satoshi locations. `arb` communicates with `bitcoind` via RPC.

If `bitcoind` is run locally by the same user, without additional
configuration, `arb` should find it automatically by reading the `.cookie` file
from `bitcoind`'s datadir, and connecting using the default RPC port.

If `bitcoind` is not on mainnet, is not run by the same user, has a non-default
datadir, or a non-default port, you'll need to pass additional flags to `arb`.
See `arb --help` for details.

Logging
--------

`arb` uses [env_logger](https://docs.rs/env_logger/latest/env_logger/). Set the
`RUST_LOG` environment variable in order to turn on logging. For example, run
the server and show `info`-level log messages and above:

```
$ RUST_LOG=info cargo run server
```

Logo
------

The `arb` logo is ◉, which is the Unicode "Fisheye" character with Unicode
codepoint `U+25C9`. Other representations include HTML (decimal) `&#9673;`, HTML (hex) `&#x25C9`,
CSS-code `\0025C9`, and JavaScript code `\u25C9`. It should ideally be displayed using the font
color `#F7931A`, but when that isn't possible (e.g., on social media posts), using the default
character in a black or white font color is acceptable as a fallback logo.

A PNG version of the logo (`logo-1000x1000.png`), in font color `#F7931A`, has also been included
for use where needed.

New Releases
------------

Release commit messages use the following template:

```
Release x.y.z

- Bump version: x.y.z → x.y.z
- Update changelog
- Update dependencies
- Update database schema version
```

Acknowledgements
------------

This repository is based on the great work done in `ord`: https://github.com/casey/ord
