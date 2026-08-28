#!/bin/sh
# One-time: database `earth` + user `earth` on postgreputest, then attach
# to app `fossall` as EARTH_DATABASE_URL (does not overwrite DATABASE_URL).
# Never prints connection URLs or `fly mpg status --json`.
#
# MPG commands need an org-scoped token (`fly tokens create org` or
# `fly auth login`). App deploy tokens cannot see the cluster.
set -eu
CLUSTER="${MPG_CLUSTER:-q49ypo4wvmzr17ln}"
CLUSTER_NAME="${MPG_CLUSTER_NAME:-postgreputest}"
APP="${FLY_APP:-fossall}"
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"

if [ -n "${FLY_ORG_TOKEN:-}" ]; then
    FLY_API_TOKEN="$FLY_ORG_TOKEN"
    export FLY_API_TOKEN
fi

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
trap 'rm -rf "$work"' EXIT

ORG="${FLY_ORG:-}"
if [ -z "$ORG" ] && command -v python3 >/dev/null 2>&1; then
    if "$FLY" apps show "$APP" --json >"$work/app.json" 2>"$work/app.err"; then
        ORG="$(python3 - "$work/app.json" <<'PY'
import json, sys
from pathlib import Path
data = json.loads(Path(sys.argv[1]).read_text())
org = data.get("Organization") or data.get("organization") or {}
slug = org.get("Slug") or org.get("slug") or org.get("RawSlug") or org.get("raw_slug") or ""
print(slug)
PY
)"
    fi
fi
if [ -n "$ORG" ]; then
    echo "Using Fly org $ORG (from FLY_ORG or app $APP)."
    set -- -o "$ORG"
else
    set --
fi

# Prefer the live cluster named postgreputest over a hardcoded ID.
if command -v python3 >/dev/null 2>&1; then
    if "$FLY" mpg "$@" list --json >"$work/mpg.json" 2>"$work/mpg.err"; then
        resolved="$(python3 - "$work/mpg.json" "$CLUSTER_NAME" "$CLUSTER" <<'PY'
import json, sys
from pathlib import Path
raw = json.loads(Path(sys.argv[1]).read_text())
want_name, fallback = sys.argv[2], sys.argv[3]
if isinstance(raw, dict):
    rows = raw.get("data") or raw.get("clusters") or raw.get("Data") or []
else:
    rows = raw
rows = rows or []
by_name = None
by_id = None
for row in rows:
    cid = row.get("id") or row.get("Id") or row.get("ID") or ""
    name = row.get("name") or row.get("Name") or ""
    if name == want_name:
        by_name = cid
    if cid == fallback:
        by_id = cid
print(by_name or by_id or "")
PY
)"
        if [ -n "$resolved" ]; then
            CLUSTER="$resolved"
            echo "Using MPG cluster $CLUSTER_NAME ($CLUSTER)."
        else
            echo "fly mpg list did not include $CLUSTER_NAME; will try $CLUSTER." >&2
        fi
    else
        echo "fly mpg list failed. App-scoped deploy tokens cannot manage MPG." >&2
        redact < "$work/mpg.err" >&2
        echo "Use an org token (fly tokens create org) as FLY_ORG_TOKEN, or run this script after fly auth login." >&2
        exit 1
    fi
fi

mpg() {
    if [ -n "$ORG" ]; then
        "$FLY" mpg -o "$ORG" "$@"
    else
        "$FLY" mpg "$@"
    fi
}

ok_or_exists() {
    what="$1"
    shift
    if out="$("$@" 2>&1)"; then
        echo "$what created."
        return 0
    fi
    lowered=$(printf '%s' "$out" | tr '[:upper:]' '[:lower:]')
    case "$lowered" in
        *already*|*exist*|*duplicate*)
            echo "$what already exists."
            ;;
        *)
            printf '%s\n' "$out" | redact >&2
            return 1
            ;;
    esac
}

ok_or_exists "Database earth" mpg databases create "$CLUSTER" -n earth
ok_or_exists "User earth" mpg users create "$CLUSTER" -u earth -r writer

echo "Applying schema to database earth…"
sql_out="$work/sql"
if ! mpg connect "$CLUSTER" -d earth < "$ROOT/db/earth/001_init.sql" >"$sql_out" 2>&1; then
    redact < "$sql_out" >&2
    exit 1
fi
redact < "$sql_out"
echo "Schema applied to database earth."

attach_out="$work/attach"
if mpg attach "$CLUSTER" -a "$APP" -d earth -u earth \
    --variable-name EARTH_DATABASE_URL >"$attach_out" 2>&1; then
    echo "Attached database earth to $APP as EARTH_DATABASE_URL (URL not printed)."
else
    if grep -qiE 'already has EARTH_DATABASE_URL' "$attach_out"; then
        echo "EARTH_DATABASE_URL already set on $APP."
    else
        redact < "$attach_out" >&2
        exit 1
    fi
fi

echo "Next (local seed): fly mpg proxy $CLUSTER"
echo "Then: ./scripts/earth-mpg-local-url.sh && make seed-earth"
echo "Do not attach without --variable-name EARTH_DATABASE_URL (that would overwrite DATABASE_URL)."
