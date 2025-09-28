# rCAT CLI

A simple CLI that allows you to issue and revoke rCATs. Works with the Sage wallet RPC.

## Usage

When using this CLI, always run Sage while logged in to the right account. You may start the RPC by going to Settings > Advanced > RPC Server and selecting 'Start.'

### Check RPC connection

This ensures the CLI can communicate with your Sage wallet.

```bash
rcli ping
```

### Create Medieval Vault

Before issuing a rCAT, you need to mint a vault. The [rCAT's TAIL](https://github.com/greimela/chia-blockchain/blob/b29d87fcbecf817bb0eda9c4bd8e823facf5a359/chia/wallet/revocable_cats/everything_with_singleton.clsp) allows the vault to mint (and melt) rCATs. The hidden puzzle hash of rCATs issued by the CLI is set so the vault's coins can also revoke rCATs.

```bash
rcli launch-vault --testnet11
```

Save the launcher id somewhere safe - this is public information, so no need to treat is as a password. You'll need the launcher id to issue and revoke rCATs.

### Issue rCATs

Your vault may issue the rCATs at any point. Currently, the CLI uses a hardcoded nonce of 0 - this means that your vault will always isse the same rCAT (same asset id and same hidden puzzle hash).

```bash
rcli issue --launcher-id [launcher-id] --cat-amount 420.0 --fee 0.0042 --testnet11
```

### Revoke rCATs

You can revoke any rCAT issued by your vault using the following command:

```bash
rcli revoke --launcher-id [launcher-id] --coin-ids [coin-ids] --fee 0.0042 --testnet11
```