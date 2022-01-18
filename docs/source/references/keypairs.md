# About

Iroha uses Public key cryptography.

# TL;DR

The easiest way to generate keys for use in Iroha is to use
`iroha_crypto_cli`.

```bash
cargo run --bin iroha_crypto_cli -- --json
```

Which produces (for example)

```json
"public_key": "ed0120bd15eea359982041771da5567c1fcb761aa22f397ad326a79246d2fd97d91b70",
"private_key": {
	"digest_function": "ed25519",
	"payload": "5b8b080aa327c403e81312f32a985635b52e2a7f88d034f41caf504a4e1743eabd15eea359982041771da5567c1fcb761aa22f397ad326a79246d2fd97d91b70"
}
```

Which you then use in `config.json` and `genesis.json`.

# More in-depth.

## Primer on public key cryptography

Public key cryptography implies the creation of a pair of keys: a public key that can be shared and a private key that is not shared and only one object has access to.

Using the private key, one can encrypt any digital data, and anyone with a public key which corresponds to that particular private key can verify it.

Iroha needs perform a *handshake*, in which the peers exchange a message encrypted with their specific private key.
So it is necessary to specify the public keys of those peers inside the configuration file `config.json` as the `TRUSTED_PEERS` (see [the configuration reference](config.md) for more details).
Moreover, in the *genesis* block, one needs to create a *genesis account*, a super user account that has elevated privileges, but only during the genesis round.
This account needs to be signed by one of the peers, in order to work.

## How to

A typical key pair has a `public_key` and a `private_key` part. The public key can be shared freely and can occur in more than one place in the network. The private key should occur only once.

The `payload** is the private key itself. The digest_function you can ignore for now.

**NB:** if you're planning on using the `private_key` in a programming language, it should be noted that both the `payload` of the `private_key` and the string representation of the `public_key** are in hexadecimal. Keys are commonly encoded using ASCII characters instead. The keys presented here should be 64 **bytes** in length.


### What needs signing.

Let's look at an [example configuration](../../../configs/peer/config.json). A peer has a `PUBLIC_KEY` and a corresponding `PRIVATE_KEY`, which must come from a single pair generated by `iroha_crypto_cli`. Every peer that wants to connect to it from the outside must know its `PRIVATE_KEY` specified in the `TRUSTED_PEERS` section.

Now look at [`genesis.json`](../../../configs/peer/genesis.json). We register an account "alice@wonderland", which has a signatory, this is not the genesis account, it's just **an** account with a key, so matching this signature to the genesis account's key is not necessary. You *could*, though and that wouldn't compromise the blockchain's security as the genesis account only has `root` user privileges for the duration of the genesis round.

The genesis account is specified in the [`config.json`](../../../configs/peer/config.json), and has nothing to do with Alice. Note also that the genesis account is signed by the peer: it has the public key of the peer. This means that the configuration specified in `config.json` is the configuration of the leader, which will submit the genesis.

So far so good. Now, only the signatory of the [`genesis.json`](../../../configs/peer/genesis.json)'s "alice@wonderland" account can use that account and interact with the blockchain. In order to do that you need to create a [client configuration](../../../configs/client_cli/config.json**. When writing the client configuration you should make sure that it references an account that is already in the network: it should have the same name, domain, public key and a private key corresponding to it.

It should go without saying, but is important to note, nonetheless, that if you lose the private key, finding a private key that corresponds to the specific public key is hard, that's why it's called a **private** key, this is also why you shouldn't duplicate your keys and keep them in plain text.