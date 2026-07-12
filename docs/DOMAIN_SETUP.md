# Custom domain: fossall.com (Cloudflare + Fly.io)

Connect the Cloudflare-managed domain `fossall.com` to the Fly.io app `fossall`.

## Prerequisites

- Domain active in Cloudflare (zone for `fossall.com`)
- Fly CLI installed and authenticated (`fly auth login`)
- App deployed (`fly deploy` / app name `fossall`)

## 1. Certificates on Fly

```bash
fly certs add fossall.com -a fossall
fly certs add www.fossall.com -a fossall
fly certs list -a fossall
```

## 2. App IP addresses

```bash
fly ips list -a fossall
```

If you have no public IPv4:

```bash
fly ips allocate-v4 --shared -a fossall
```

Note the **v4** and **v6** addresses for DNS.

## 3. Cloudflare DNS

Dashboard: [Cloudflare](https://dash.cloudflare.com/) → `fossall.com` → **DNS** → **Records**.

| Type | Name | Content | Proxy status |
|------|------|---------|--------------|
| `A` | `@` | Fly IPv4 | **DNS only** (grey cloud) |
| `AAAA` | `@` | Fly IPv6 | **DNS only** (grey cloud) |
| `CNAME` | `www` | `fossall.com` | **DNS only** (grey cloud) |

### Important: proxy off for cert issuance

For Fly’s Let’s Encrypt validation to work cleanly, use **DNS only** (grey cloud), not the orange Cloudflare proxy.

| Setting | Value |
|---------|-------|
| Proxy | DNS only (grey) |
| SSL/TLS (if you later enable orange proxy) | Full (strict) |

Fly terminates TLS when DNS is grey-clouded.

## 4. Verify

```bash
fly certs check fossall.com -a fossall
dig fossall.com A +short
dig fossall.com AAAA +short
curl -I https://fossall.com/
curl -I https://www.fossall.com/
```

Expected: certificate issued; HTTPS 200 from the app.

## Optional: www → apex redirect

Cloudflare → **Rules** → **Redirect Rules**:

- When: hostname equals `www.fossall.com`
- Then: dynamic redirect to `https://fossall.com${uri}` (301)

## Troubleshooting

```bash
# DNS not pointing at Fly
dig fossall.com A +short

# Force re-check / re-add cert
fly certs show fossall.com -a fossall
fly certs remove fossall.com -a fossall
fly certs add fossall.com -a fossall
```

## Useful commands

```bash
fly certs list -a fossall
fly certs show DOMAIN -a fossall
fly certs check DOMAIN -a fossall
fly ips list -a fossall
```

## References

- [Fly custom domains](https://fly.io/docs/networking/custom-domains/)
- [Cloudflare DNS](https://developers.cloudflare.com/dns/)
