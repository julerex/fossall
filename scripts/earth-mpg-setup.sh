#!/bin/sh
# One-time: ensure database `earth` on postgreputest, apply schema, attach
# to app `fossall` as EARTH_DATABASE_URL (does not overwrite DATABASE_URL).
# Never prints connection URLs or `fly mpg status --json`.
#
# `fly mpg databases create` and SQL CREATE DATABASE both fail on this
# MPG v1 cluster with the deploy token (API 404 / permission denied).
# After `earth` exists (Fly dashboard Databases tab, or a token that can
# create DBs), this script applies schema and attaches.
set -eu
CLUSTER="${MPG_CLUSTER:-q49ypo4wvmzr17ln}"
CLUSTER_NAME="${MPG_CLUSTER_NAME:-postgreputest}"
APP="${FLY_APP:-fossall}"
DB_NAME="${EARTH_DB_NAME:-earth}"
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"

if [ -n "${FLY_ORG_TOKEN:-}" ]; then
    FLY_API_TOKEN="$FLY_ORG_TOKEN"
    export FLY_API_TOKEN
fi
# FLY_ORG injects --org into mpg subcommands that reject it (create/users).
unset FLY_ORG || true

redact() {
    sed -E 's#postgres(ql)?://[^[:space:]]+#postgres://***#g'
}

if ! command -v fly >/dev/null 2>&1 && ! command -v flyctl >/dev/null 2>&1; then
    echo "fly/flyctl is required" >&2
    exit 1
fi
FLY="$(command -v fly || true)"
if [ -z "$FLY" ]; then
    FLY="$(command -v flyctl)"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"; [ -n "${proxy_pid:-}" ] && kill "$proxy_pid" 2>/dev/null || true' EXIT

ORG="${FLY_ORG_SLUG:-}"
if [ -z "$ORG" ] && command -v python3 >/dev/null 2>&1; then
    extract_org="$work/extract_org.py"
    cat >"$extract_org" <<'PY'
import json, sys
from pathlib import Path

app = sys.argv[1]
data = json.loads(Path(sys.argv[2]).read_text())

def slug_from(org):
    if isinstance(org, str) and org.strip():
        return org.strip()
    if isinstance(org, dict):
        for k in ("Slug", "slug", "RawSlug", "raw_slug", "Name", "name"):
            v = org.get(k)
            if isinstance(v, str) and v.strip():
                return v.strip()
    return ""

def walk(obj):
    if isinstance(obj, dict):
        name = obj.get("Name") or obj.get("name") or obj.get("ID") or obj.get("id")
        if name == app:
            s = slug_from(obj.get("Organization") or obj.get("organization"))
            if s:
                return s
        for v in obj.values():
            s = walk(v)
            if s:
                return s
    elif isinstance(obj, list):
        for item in obj:
            s = walk(item)
            if s:
                return s
    return ""

print(walk(data) or slug_from(
    data.get("Organization") or data.get("organization") if isinstance(data, dict) else None
))
PY
    for cmd in \
        "$FLY apps list --json" \
        "$FLY status -a $APP --json" \
        "$FLY apps info -a $APP --json"
    do
        # shellcheck disable=SC2086
        if eval "$cmd" >"$work/org.json" 2>"$work/org.err"; then
            got="$(python3 "$extract_org" "$APP" "$work/org.json" 2>/dev/null || true)"
            if [ -n "$got" ]; then
                ORG="$got"
                echo "Resolved Fly org $ORG via: $cmd"
                break
            fi
        fi
    done
fi
if [ -z "$ORG" ]; then
    echo "Could not resolve Fly org for app $APP (set FLY_ORG_SLUG)." >&2
    exit 1
fi
echo "Using Fly org $ORG."

# -o only on `mpg list`.
if command -v python3 >/dev/null 2>&1; then
    if "$FLY" mpg -o "$ORG" list --json >"$work/mpg.json" 2>"$work/mpg.err"; then
        resolved="$(python3 - "$work/mpg.json" "$CLUSTER_NAME" "$CLUSTER" <<'PY'
import json, sys
from pathlib import Path
raw = json.loads(Path(sys.argv[1]).read_text())
want_name, fallback = sys.argv[2], sys.argv[3]
rows = raw.get("data") if isinstance(raw, dict) else raw
rows = rows or []
chosen = None
for row in rows:
    cid = row.get("id") or row.get("Id") or row.get("ID") or ""
    name = row.get("name") or row.get("Name") or ""
    if name == want_name or cid == fallback:
        chosen = row
        if name == want_name:
            break
if not chosen:
    raise SystemExit(0)
print(chosen.get("id") or chosen.get("Id") or chosen.get("ID") or "")
ver = chosen.get("version") or chosen.get("Version") or ""
mpgd = chosen.get("mpgd_cluster_id") or chosen.get("MpgdClusterId") or ""
print("cluster version:", ver, file=sys.stderr)
print("mpgd_cluster_id set:", "yes" if mpgd else "no", file=sys.stderr)
PY
)"
        if [ -n "$resolved" ]; then
            CLUSTER="$resolved"
            echo "Using MPG cluster $CLUSTER_NAME ($CLUSTER)."
        else
            echo "fly mpg list did not include $CLUSTER_NAME; will try $CLUSTER." >&2
        fi
    else
        echo "fly mpg list failed; will try documented cluster $CLUSTER." >&2
        redact < "$work/mpg.err" >&2
    fi
fi

mpg() {
    env -u FLY_ORG "$FLY" mpg "$@"
}

echo "Trying control-plane database create (no --org)…"
if mpg databases create "$CLUSTER" -n "$DB_NAME" >"$work/createdb.cli" 2>&1; then
    echo "fly mpg databases create succeeded."
else
    echo "fly mpg databases create did not create $DB_NAME:"
    redact < "$work/createdb.cli"
fi

if mpg databases list "$CLUSTER" >"$work/dblist" 2>&1; then
    echo "Databases on cluster (names only):"
    redact < "$work/dblist"
else
    echo "fly mpg databases list failed (v1 clusters without the role-system opt-in 404 here)."
    redact < "$work/dblist"
fi

if mpg users list "$CLUSTER" >"$work/userlist" 2>&1; then
    echo "Users on cluster (names/roles only):"
    redact < "$work/userlist"
    if mpg users create "$CLUSTER" -u earth -r writer >"$work/usercreate" 2>&1; then
        echo "Created MPG user earth (writer)."
    else
        echo "fly mpg users create earth skipped:"
        redact < "$work/usercreate"
    fi
else
    echo "fly mpg users list failed (same v1/role-system limitation as databases)."
    redact < "$work/userlist"
fi

admin_url_file="$work/admin.url"
earth_exists=0
if mpg status "$CLUSTER" --json >"$work/status.json" 2>"$work/status.err"; then
    python3 - "$work/status.json" "$admin_url_file" <<'PY'
import json, sys, urllib.parse
from pathlib import Path
data = json.loads(Path(sys.argv[1]).read_text())
creds = data.get("credentials") or {}
user, password = creds.get("user") or "", creds.get("password") or ""
if not user or not password:
    sys.stderr.write("status json missing credentials\n")
    sys.exit(1)
dbname = creds.get("dbname") or creds.get("DBName") or "postgres"
url = (
    "postgres://"
    + urllib.parse.quote(user, safe="")
    + ":"
    + urllib.parse.quote(password, safe="")
    + "@127.0.0.1:16380/"
    + dbname
)
Path(sys.argv[2]).write_text(url)
Path(sys.argv[2]).chmod(0o600)
PY
    echo "Got cluster credentials via mpg status (not printed)."
else
    echo "mpg status --json failed:" >&2
    redact < "$work/status.err" >&2
fi

# Prefer schema_admin fly-user when the credentials endpoint allows it.
if [ -n "${FLY_API_TOKEN:-}" ]; then
    cred_code="$(curl -sS -o "$work/flyuser.json" -w '%{http_code}' \
        -H "Authorization: Bearer ${FLY_API_TOKEN}" \
        -H "Content-Type: application/json" \
        "https://api.fly.io/api/v1/postgres/${CLUSTER}/users/fly-user/credentials" || echo 000)"
    echo "GET fly-user credentials -> HTTP $cred_code"
    if [ "$cred_code" = "200" ] && command -v python3 >/dev/null 2>&1; then
        if python3 - "$work/flyuser.json" "$admin_url_file" <<'PY'
import json, sys, urllib.parse
from pathlib import Path
raw = json.loads(Path(sys.argv[1]).read_text())
data = raw.get("data") if isinstance(raw, dict) else {}
data = data or {}
user = data.get("user") or data.get("User") or ""
password = data.get("password") or data.get("Password") or ""
if not user or not password:
    sys.stderr.write("fly-user credentials json missing user/password\n")
    sys.exit(1)
url = (
    "postgres://"
    + urllib.parse.quote(user, safe="")
    + ":"
    + urllib.parse.quote(password, safe="")
    + "@127.0.0.1:16380/fossall"
)
Path(sys.argv[2]).write_text(url)
Path(sys.argv[2]).chmod(0o600)
PY
        then
            echo "Using fly-user (schema_admin) for SQL (URL not printed)."
        fi
    fi
    rm -f "$work/flyuser.json"
fi

if [ -f "$admin_url_file" ]; then
    if ! command -v psql >/dev/null 2>&1; then
        echo "psql is required (postgresql-client)" >&2
        exit 1
    fi
    mpg proxy "$CLUSTER" --bind-addr 127.0.0.1 --local-port 16380 >/dev/null 2>&1 &
    proxy_pid=$!
    export PGSSLMODE=prefer
    export PGGSSENCMODE=disable
    admin_url="$(cat "$admin_url_file")"
    n=0
    while [ "$n" -lt 20 ]; do
        if psql "$admin_url" -c 'SELECT 1' >"$work/psql.wait" 2>&1; then
            break
        fi
        n=$((n + 1))
        sleep 1
    done
    if psql "$admin_url" -v ON_ERROR_STOP=1 -c 'SELECT current_user, current_database();' \
        >"$work/whoami" 2>&1; then
        echo "Connected via proxy as:"
        redact < "$work/whoami"
    else
        echo "psql whoami failed:" >&2
        redact < "$work/whoami" >&2
        redact < "$work/psql.wait" >&2 || true
    fi
    if psql "$admin_url" -tAc \
        "SELECT datname FROM pg_database WHERE datname = '$DB_NAME'" \
        >"$work/dbprobe" 2>&1; then
        if grep -qx "$DB_NAME" "$work/dbprobe" >/dev/null 2>&1; then
            earth_exists=1
            echo "Database $DB_NAME already exists."
        fi
    else
        echo "Could not list pg_database:" >&2
        redact < "$work/dbprobe" >&2
    fi
    if [ "$earth_exists" -eq 0 ]; then
        echo "Trying SQL CREATE DATABASE $DB_NAME…"
        if psql "$admin_url" -v ON_ERROR_STOP=1 -c "CREATE DATABASE $DB_NAME;" \
            >"$work/psql.createdb" 2>&1 \
            || grep -qiE 'already exists' "$work/psql.createdb"; then
            earth_exists=1
            echo "Database $DB_NAME ensured via psql."
        else
            echo "SQL CREATE DATABASE failed (expected on MPG writer roles):"
            redact < "$work/psql.createdb"
        fi
    fi
    if [ "$earth_exists" -eq 1 ]; then
        echo "Applying schema to database $DB_NAME via psql…"
        ADMIN_URL="$admin_url"
        export ADMIN_URL
        earth_url="$(DB_NAME="$DB_NAME" python3 - <<'PY'
import os, urllib.parse
u = urllib.parse.urlparse(os.environ["ADMIN_URL"])
print(urllib.parse.urlunparse((u.scheme, u.netloc, "/" + os.environ["DB_NAME"], "", "", "")))
PY
)"
        unset ADMIN_URL
        if psql "$earth_url" -v ON_ERROR_STOP=1 -f "$ROOT/db/earth/001_init.sql" \
            >"$work/sql" 2>&1; then
            echo "Schema applied to database $DB_NAME."
        else
            echo "psql schema apply failed:" >&2
            redact < "$work/sql" >&2
            unset admin_url earth_url
            exit 1
        fi
        unset admin_url earth_url
    fi
    rm -f "$work/status.json" "$admin_url_file"
    kill "$proxy_pid" 2>/dev/null || true
    proxy_pid=""
fi

if [ "$earth_exists" -eq 0 ]; then
    cat >&2 <<EOF
Database $DB_NAME does not exist on $CLUSTER_NAME.

\`fly mpg databases list\` works (fly-db, fossall, …) but create 404s with the
app deploy token. SQL CREATE DATABASE is denied for writer roles.

Either:
  A) GitHub secret FLY_ORG_TOKEN from \`fly tokens create org\`, then re-run, or
  B) One-time in the Fly dashboard:
     https://fly.io/dashboard/${ORG}/managed_postgres/${CLUSTER}
     Databases tab: create a database named ${DB_NAME}
     Optional: Users tab → user ${DB_NAME}, role writer
     Then re-run this script / Earth MPG setup workflow_dispatch

Do not create a second MPG cluster. Do not fly mpg attach without
--variable-name EARTH_DATABASE_URL.
EOF
    exit 1
fi

attach_out="$work/attach"
if mpg attach "$CLUSTER" -a "$APP" -d "$DB_NAME" \
    --variable-name EARTH_DATABASE_URL >"$attach_out" 2>&1; then
    echo "Attached database $DB_NAME as EARTH_DATABASE_URL (URL not printed)."
elif grep -qiE 'already has EARTH_DATABASE_URL' "$attach_out"; then
    echo "EARTH_DATABASE_URL already set on $APP."
else
    redact < "$attach_out" >&2
    exit 1
fi

echo "Next (local seed): fly mpg proxy $CLUSTER"
echo "Then: ./scripts/earth-mpg-local-url.sh && make seed-earth"
echo "Do not attach without --variable-name EARTH_DATABASE_URL (that would overwrite DATABASE_URL)."
