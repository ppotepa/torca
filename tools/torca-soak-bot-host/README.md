# Torca soak bot host

This is a development-only supervisor for persistent soak participants. It
starts one `torca-lab-peer` process per bot and keeps each profile under a
stable directory. Pairing therefore happens during provisioning, not at the
start of every battery measurement.

The HTTP surface is deliberately tiny:

```text
GET  /health
POST /bot/peer-a   (body: one torca-lab-peer JSONL request)
```

Every request requires `X-Torca-Soak-Token`. Bind it to loopback or a private
Docker network only. Do not use this service in a production deployment and do
not put normal client profiles, keys, or message content in its root.

For the Docker development stack:

```powershell
$env:TORCA_RELAY_ENDPOINT = '<current-onion>:443'
$env:TORCA_SOAK_BOT_TOKEN = '<random-dev-token-at-least-16-chars>'
docker compose -f infra/docker/compose.yml -f infra/docker/compose.soak.yml up --build soak-bot-host
```
