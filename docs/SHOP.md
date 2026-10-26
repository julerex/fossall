# Simulated shop

`/shop` is a fake storefront on fossall.com. Visitors browse a fixed catalog without an account. Cart, checkout, and order history require **Sign in with X**. Checkout writes an order and clears the cart. There is no card form, no address, and no payment provider.

Every shop page shows: **Simulation. Nothing is charged or shipped.**

Shop data lives in the existing Fossall database:

| | |
|---|---|
| Database | `fossall` (same as `/words`) |
| Schema | `shop` |
| App user | `fossall` |
| App env | `DATABASE_URL` |

Do **not** create a second Managed Postgres cluster, add a Fly volume, or `fly mpg attach` a second database (attach would overwrite `DATABASE_URL`).

If `DATABASE_URL` is unset, `/shop` returns 503. `/health` never touches Postgres. If the X client id, client secret, or session secret is unset, the catalog still loads and the sign-in control explains that login is not configured.

## Schema

```bash
fly mpg connect q49ypo4wvmzr17ln -d fossall < db/002_shop.sql
```

Tables: `shop.users`, `shop.products`, `shop.cart_items`, `shop.orders`, `shop.order_items`. Products are seeded in that file. Re-applying updates names and prices by `sku`.

## X developer portal (manual)

Register a confidential web app on the X developer portal. Callback URLs must match exactly:

- `https://fossall.com/auth/x/callback`
- `http://localhost:8080/auth/x/callback`

The app uses OAuth 2.0 authorization-code with Proof Key for Code Exchange (`S256`). Scopes: `users.read` and `tweet.read` only. No `offline.access`, no write scopes. After `GET https://api.twitter.com/2/users/me`, the access token is dropped and never stored.

## Secrets

Set on the Fly app with `fly secrets set`. Never commit them. Never log them.

| Variable | Purpose |
|---|---|
| `X_CLIENT_ID` | X OAuth 2.0 client id |
| `X_CLIENT_SECRET` | X OAuth 2.0 client secret |
| `SHOP_SESSION_SECRET` | Long random value; the server derives the encrypted cookie key so either Fly machine can read sessions after a cold start |
| `PUBLIC_BASE_URL` | `https://fossall.com` in production, `http://localhost:8080` locally |

Local development (shop catalog needs the same proxy as `/words`):

```bash
fly mpg proxy q49ypo4wvmzr17ln
export DATABASE_URL='postgres://…@localhost:16380/fossall'
export PUBLIC_BASE_URL='http://localhost:8080'
export X_CLIENT_ID='…'
export X_CLIENT_SECRET='…'
export SHOP_SESSION_SECRET='…'
make dev
# http://localhost:8080/shop
```

Sign-in cookies are `HttpOnly` and `SameSite=Lax`. `Secure` is on when `PUBLIC_BASE_URL` starts with `https://`.

## Routes

- `GET /shop` — catalog
- `GET /shop/{sku}` — product
- `GET`/`POST /shop/cart` — cart (signed in)
- `POST /shop/checkout` — simulated order
- `GET /shop/orders` and `GET /shop/orders/{id}` — that user's history
- `GET /auth/x` and `GET /auth/x/callback` — X login
- `POST /auth/logout` — clear the session cookie
