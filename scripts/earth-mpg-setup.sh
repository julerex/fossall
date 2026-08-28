#!/bin/sh
# One-time: database `earth` + user `earth` on postgreputest, then attach
# to app `fossall` as EARTH_DATABASE_URL (does not overwrite DATABASE_URL).
# Never prints connection URLs or `fly mpg status --json`.
set -eu
CLUSTER="${MPG_CLUSTER:-q49ypo4wvmzr17ln}"
APP="${FLY_APP:-fossall}"
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"

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

ok_or_exists "Database earth" "$FLY" mpg databases create "$CLUSTER" -n earth
ok_or_exists "User earth" "$FLY" mpg users create "$CLUSTER" -u earth -r writer

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "Applying schema to database earth…"
sql_out="$work/sql"
if ! "$FLY" mpg connect "$CLUSTER" -d earth < "$ROOT/db/earth/001_init.sql" >"$sql_out" 2>&1; then
    redact < "$sql_out" >&2
    exit 1
fi
redact < "$sql_out"
echo "Schema applied to database earth."

attach_out="$work/attach"
if "$FLY" mpg attach "$CLUSTER" -a "$APP" -d earth -u earth \
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
