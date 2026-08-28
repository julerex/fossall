# Fossall Postgres

Fossall reads five-letter English words from Fly.io Managed Postgres cluster **postgreputest** (`q49ypo4wvmzr17ln`, region `fra`). The cluster is shared with other apps in the org. Fossall data lives only in:

| | |
|---|---|
| Database | `fossall` |
| Schema | `words` |
| Table | `words.five_letter_words` |
| App user | `fossall` (MPG role `writer`) |

Do **not** create a second MPG cluster, add a Fly volume, allocate a dedicated IPv4, or raise `min_machines_running`. Do **not** paste `fly mpg status --json` (it prints credentials) into chat, commits, or issues.

Word list: `data/five_letter_words.txt`, filtered from [ENABLE 1.0](https://en.wikipedia.org/wiki/ENABLE_(word_list)) (public domain): lowercase ASCII `[a-z]{5}` only.

## One-time setup

```bash
fly mpg databases create q49ypo4wvmzr17ln -n fossall
fly mpg users create q49ypo4wvmzr17ln -u fossall -r writer

# Schema (psql via platform auth — no URL in the shell history)
fly mpg connect q49ypo4wvmzr17ln -d fossall < db/001_init.sql

# Seed. Proxy first, then set DATABASE_URL to the fossall database
# on localhost (pooled or direct). Never commit the URL.
fly mpg proxy q49ypo4wvmzr17ln
export DATABASE_URL='postgres://…@localhost:16380/fossall'
make seed

# Inject DATABASE_URL on the Fly app (restarts Machines).
fly mpg attach q49ypo4wvmzr17ln -a fossall -d fossall -u fossall
```

## Local `/words`

Postgres is not on the public internet. Other pages work without a database.

```bash
fly mpg proxy q49ypo4wvmzr17ln
export DATABASE_URL='postgres://…@localhost:16380/fossall'
make dev
# http://localhost:8080/words
```

If `DATABASE_URL` is unset, the server still starts; `/words` returns 503 with an explanation. `/health` never touches Postgres.

## Earth database (second database on the same cluster)

Deep-time globe data lives in a **separate** database on the same cluster. Do **not** create a second cluster. Attach with `--variable-name EARTH_DATABASE_URL` so the words `DATABASE_URL` is not overwritten.

| | |
|---|---|
| Database | `earth` |
| Schema | `earth` |
| App user | `earth` (MPG role `writer`) |
| App env | `EARTH_DATABASE_URL` |

```bash
./scripts/earth-mpg-setup.sh
# Applies db/earth/001_init.sql, then:
# fly mpg attach q49ypo4wvmzr17ln -a fossall -d earth \
#   --variable-name EARTH_DATABASE_URL

# Local seed through the MPG proxy (URL is written to a file, never echoed):
fly mpg proxy q49ypo4wvmzr17ln
./scripts/earth-mpg-local-url.sh
export EARTH_DATABASE_URL="$(cat "${TMPDIR:-/tmp}/earth-database-url")"
make seed-earth
```

Do **not** `fly mpg attach` without `--variable-name` (default secret name is `DATABASE_URL`).

### One-time: create database `earth`

postgreputest is **MPG v1**. `fly mpg databases list` / `users list` work with the deploy token (databases today: `fly-db`, `fossall`, `reputest`, `timehelm`; users include `fly-user` schema_admin and `fossall` writer). `fly mpg databases create` / `users create` return 404 with that token, and the writer role cannot `CREATE DATABASE`.

Either add GitHub secret **`FLY_ORG_TOKEN`** (`fly tokens create org`) and re-run **Earth MPG setup**, or create the database once in the UI:

1. Open [postgreputest in the Fly dashboard](https://fly.io/dashboard/personal/managed_postgres/q49ypo4wvmzr17ln)
2. **Databases** tab: create a database named `earth`
3. Optional: **Users** tab → user `earth`, role `writer`
4. Re-run `.github/workflows/earth-mpg-setup.yml` (workflow_dispatch is fine)

Do **not** create a second cluster. Sources, licenses, and API: [EARTH.md](EARTH.md).

If `EARTH_DATABASE_URL` is unset, `/earth` still renders; `/api/earth/*` returns 503. `/health` never touches Postgres.
