# Eth-Alive

A lightweight daemon that monitors Ethereum node health. It compares a local node against a remote peer, triggering Discord alerts if the node falls behind or becomes unresponsive.

## Installation

### Docker

```bash
docker run -d \
  --name eth-alive \
  --restart always \
  -e LOCAL_RPC_URL="http://host.docker.internal:8545" \
  -e REMOTE_RPC_URL="https://ethereum.publicnode.com" \
  -e DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/..." \
  alamiinsi/eth-alive:latest
```

### Cargo

```bash
cargo install eth-alive
```

### Pre-compiled Binaries

Download the latest release from the [Releases Page](https://github.com/MantissaMr/eth-alive/releases).

## Configuration

eth-alive is configured via **Environment Variables**.

| Variable | Description | Default |
|----------|-------------|---------|
| `LOCAL_RPC_URL` | Required. The HTTP endpoint of the node being monitored. | N/A |
| `REMOTE_RPC_URL` | Required. The HTTP endpoint of a trusted public node. | N/A |
| `DISCORD_WEBHOOK_URL` | Required. The webhook URL where alerts will be sent. | N/A |
| `LAG_THRESHOLD` | Block lag tolerance before alerting. | 3 |
| `ALERT_COOLDOWN_MINUTES` | Minutes to wait before sending another alert to Discord. | 15 |
| `POLL_INTERVAL_SECONDS` | How often (in seconds) to check the nodes. | 60 |

## Usage

**Running with Docker:** Pass variables using the `-e` flag (see Installation).

**Running manually:** Pass variables inline or use a `.env` file in the working directory.

```bash
LOCAL_RPC_URL="http://localhost:8545" REMOTE_RPC_URL="..." DISCORD_WEBHOOK_URL="..." eth-alive
```

## License

```
MIT License
```