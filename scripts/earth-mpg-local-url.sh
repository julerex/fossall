#!/bin/sh
# Write a localhost MPG-proxy URL for database `earth` to a file and export
# EARTH_DATABASE_URL from it. Never prints the URL or `fly mpg status --json`.
#
# Prerequisites: `fly mpg proxy $CLUSTER` listening on 127.0.0.1:16380
# (override port with EARTH_PROXY_PORT).
set -eu
CLUSTER="${MPG_CLUSTER:-q49ypo4wvmzr17ln}"
PORT="${EARTH_PROXY_PORT:-16380}"
OUT="${EARTH_URL_FILE:-${TMPDIR:-/tmp}/earth-database-url}"

if ! command -v fly >/dev/null 2>&1 && ! command -v flyctl >/dev/null 2>&1; then
    echo "fly/flyctl is required" >&2
    exit 1
fi
FLY="$(command -v fly || true)"
if [ -z "$FLY" ]; then
    FLY="$(command -v flyctl)"
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required" >&2
    exit 1
fi

json_out="$(mktemp)"
trap 'rm -f "$json_out"' EXIT
if ! "$FLY" mpg status "$CLUSTER" --json >"$json_out" 2>/dev/null; then
    echo "fly mpg status failed (output not printed)" >&2
    exit 1
fi

python3 - "$json_out" "$OUT" "$PORT" <<'PY'
import json, sys, urllib.parse
from pathlib import Path

src, dest, port = sys.argv[1], sys.argv[2], sys.argv[3]
try:
    data = json.loads(Path(src).read_text())
except Exception:
    sys.stderr.write("could not parse mpg status json\n")
    sys.exit(1)

creds = data.get("credentials") or {}
user = creds.get("user") or ""
password = creds.get("password") or ""
if not user or not password:
    sys.stderr.write("mpg status json missing credentials\n")
    sys.exit(1)

netloc = f"{urllib.parse.quote(user, safe='')}:{urllib.parse.quote(password, safe='')}@127.0.0.1:{port}"
url = urllib.parse.urlunparse(("postgres", netloc, "/earth", "", "", ""))
path = Path(dest)
path.write_text(url)
path.chmod(0o600)
PY

# Caller: export EARTH_DATABASE_URL="$(cat "$OUT")" — do not echo it.
echo "Wrote localhost earth URL to $OUT (contents not printed)."
echo "Export with: export EARTH_DATABASE_URL=\"\$(cat $OUT)\""
