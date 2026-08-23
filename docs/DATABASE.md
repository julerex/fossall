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
