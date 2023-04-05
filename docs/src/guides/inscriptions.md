Ordinal Inscription Guide
=========================

Individual sats can be inscribed with arbitrary content, creating
Bitcoin-native digital artifacts that can be held in a Bitcoin wallet and
transferred using Bitcoin transactions. Inscriptions are as durable, immutable,
secure, and decentralized as Bitcoin itself.

Working with inscriptions requires a Bitcoin full node, to give you a view of
the current state of the Bitcoin blockchain, and a wallet that can create
inscriptions and perform sat control when constructing transactions to send
inscriptions to another wallet.

Bitcoin Core provides both a Bitcoin full node and wallet. However, the Bitcoin
Core wallet cannot create inscriptions and does not perform sat control.

This requires an the ordinal-aware utility, like [`arb`](https://github.com/tyjvazum/arb). `arb`
doesn't implement its own wallet, so `arb wallet` subcommands interact with
Bitcoin Core wallets.

This guide covers:

1. Installing Bitcoin Core
2. Syncing the Bitcoin blockchain
3. Creating a Bitcoin Core wallet
4. Using `arb wallet receive` to receive sats
5. Creating inscriptions with `arb wallet inscribe`
6. Sending inscriptions with `arb wallet send`
7. Receiving inscriptions with `arb wallet receive`

Getting Help
------------

If you get stuck, try asking for help in the
[Arb Gitter room](https://app.gitter.im/#/room/#arb-proto:gitter.im), or checking the GitHub
[issues](https://github.com/tyjvazum/arb/issues) and
[discussions](https://github.com/tyjvazum/arb/discussions).

Installing Bitcoin Core
-----------------------

Bitcoin Core is available from [bitcoincore.org](https://bitcoincore.org/) on
the [download page](https://bitcoincore.org/en/download/).

Making inscriptions requires Bitcoin Core 24 or newer.

This guide does not cover installing Bitcoin Core in detail. Once Bitcoin Core
is installed, you should be able to run `bitcoind -version` successfully from
the command line.

Configuring Bitcoin Core
------------------------

`arb` requires Bitcoin Core's transaction index.

To configure your Bitcoin Core node to maintain a transaction
index, add the following to your `bitcoin.conf`:

```
txindex=1
```

Or, run `bitcoind` with `-txindex`:

```
bitcoind -txindex
```

Syncing the Bitcoin Blockchain
------------------------------

To sync the chain, run:

```
bitcoind -txindex
```

…and leave it running until `getblockcount`:

```
bitcoin-cli getblockcount
```

agrees with the block count on a block explorer like [the mempool.space block
explorer](https://mempool.space/). `arb` interacts with `bitcoind`, so you
should leave `bitcoind` running in the background when you're using `arb`.

Installing `arb`
----------------

The `arb` utility is written in Rust and can be built from
[source](https://github.com/tyjvazum/arb). Pre-built binaries are available on the
[releases page](https://github.com/tyjvazum/arb/releases).

You can install the latest pre-built binary from the command line with:

```sh
curl --proto '=https' --tlsv1.2 -fsLS https://raw.githubusercontent.com/tyjvazum/arb/master/install.sh | bash -s
```

Once `arb` is installed, you should be able to run:

```
arb --version
```

Which prints out `arb`'s version number.

Creating a Bitcoin Core Wallet
------------------------------

`arb` uses Bitcoin Core to manage private keys, sign transactions, and
broadcast transactions to the Bitcoin network.

To create a Bitcoin Core wallet named `arb` for use with `arb`, run:

```
arb wallet create
```

Receiving Sats
--------------

Inscriptions are made on individual sats, using normal Bitcoin transactions
that pay fees in sats, so your wallet will need some sats.

Get a new address from your `arb` wallet by running:

```
arb wallet receive
```

And send it some funds.

You can see pending transactions with:

```
arb wallet transactions
```

Once the transaction confirms, you should be able to see the transactions
outputs with `arb wallet outputs`.

Creating Inscription Content
----------------------------

Sats can be inscribed with any kind of content, but the `arb` wallet only
supports content types that can be displayed by the `arb` block explorer.

Additionally, inscriptions are included in transactions, so the larger the
content, the higher the fee that the inscription transaction must pay.

Inscription content is included in transaction witnesses, which receive the
witness discount. To calculate the approximate fee that an inscribe transaction
will pay, divide the content size by four and muliply by the fee rate.

Inscription transactions must be less than 400,000 weight units, or they will
not be relayed by Bitcoin Core. One byte of inscription content costs one
weight unit. Since an inscription transaction includes not just the inscription
content, limit inscription content to less than 400,000 weight units. 390,000
weight units should be safe.

Creating Inscriptions
---------------------

To create an inscription with the contents of `FILE`, run:

```
arb wallet inscribe --fee-rate FEE_RATE FILE
```

Arb will output two transactions IDs, one for the commit transaction, and one
for the reveal transaction, and the inscription ID. Inscription IDs are of the
form `TXIDiN`, where `TXID` is the transaction ID of the reveal transaction,
and `N` is the index of the inscription in the reveal transaction.

The commit transaction commits to a tapscript containing the contents of the
inscription, and the reveal transaction spends from that tapscript, revealing
the contents on chain and inscribing them on the first sat of the first output
of the reveal transaction.

Wait for the reveal transaction to be mined. You can check the status of the
commit and reveal transactions using  [the mempool.space block
explorer](https://mempool.space/).

Once the reveal transaction has been mined, the inscription ID should be
printed when you run:

```
arb wallet inscriptions
```

And when you visit a standards-compliant [ordinals explorer](https://ordinals.com/) at
`/inscription/INSCRIPTION_ID`.

Sending Inscriptions
--------------------

Ask the recipient to generate a new address by running:

```
arb wallet receive
```

Send the inscription by running:

```
arb wallet send --fee-rate <FEE_RATE> <ADDRESS> <INSCRIPTION_ID>
```

See the pending transaction with:

```
arb wallet transactions
```

Once the send transaction confirms, the recipient can confirm receipt by
running:

```
arb wallet inscriptions
```

Receiving Inscriptions
----------------------

Generate a new receive address using:

```
arb wallet receive
```

The sender can transfer the inscription to your address using:

```
arb wallet send ADDRESS INSCRIPTION_ID
```

See the pending transaction with:
```
arb wallet transactions
```

Once the send transaction confirms, you can can confirm receipt by running:

```
arb wallet inscriptions
```
