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
        "$FLY apps info -a $APP --json" \
        "$FLY orgs list --json"
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
    if [ -z "$ORG" ]; then
        echo "Could not resolve Fly org for app $APP (set FLY_ORG)." >&2
        redact < "$work/org.err" >&2 || true
    fi
fi
if [ -n "$ORG" ]; then
    echo "Using Fly org $ORG."
else
    echo "Org slug is required in non-interactive runs (set FLY_ORG or use fly apps list)." >&2
    exit 1
fi

mpg() {
    "$FLY" mpg "$@"
}

# Prefer the live cluster named postgreputest over a hardcoded ID.
# Pass -o on `mpg list` only; FLY_ORG env injects --org into create/users.
if command -v python3 >/dev/null 2>&1; then
    if "$FLY" mpg -o "$ORG" list --json >"$work/mpg.json" 2>"$work/mpg.err"; then
        resolved="$(python3 - "$work/mpg.json" "$CLUSTER_NAME" "$CLUSTER" "$work/cluster.env" <<'PY'
import json, sys
from pathlib import Path
raw = json.loads(Path(sys.argv[1]).read_text())
want_name, fallback, out_path = sys.argv[2], sys.argv[3], sys.argv[4]
if isinstance(raw, dict):
    rows = raw.get("data") or raw.get("clusters") or raw.get("Data") or []
else:
    rows = raw
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
    print("")
    raise SystemExit(0)
cid = chosen.get("id") or chosen.get("Id") or chosen.get("ID") or ""
mpgd = chosen.get("mpgd_cluster_id") or chosen.get("MpgdClusterId") or ""
ver = chosen.get("version") or chosen.get("Version") or ""
Path(out_path).write_text(f"CLUSTER_ID={cid}\nMPGD_ID={mpgd}\nCLUSTER_VERSION={ver}\n")
print(cid)
PY
)"
        if [ -n "$resolved" ]; then
            CLUSTER="$resolved"
            echo "Using MPG cluster $CLUSTER_NAME ($CLUSTER)."
            if [ -f "$work/cluster.env" ]; then
                # shellcheck disable=SC1090
                . "$work/cluster.env"
                if [ -n "${MPGD_ID:-}" ]; then
                    echo "MPGv2 id present."
                fi
            fi
        else
            echo "fly mpg list did not include $CLUSTER_NAME; will try $CLUSTER." >&2
        fi
    else
        echo "fly mpg list failed; will try documented cluster $CLUSTER." >&2
        redact < "$work/mpg.err" >&2
    fi
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

# Do not export FLY_ORG: flyctl would inject --org into create/users.
mpg_post() {
    path="$1"
    body="$2"
    auth="${FLY_API_TOKEN:-}"
    [ -n "$auth" ] || return 1
    ids="$CLUSTER"
    if [ -n "${MPGD_ID:-}" ]; then
        ids="$MPGD_ID $CLUSTER"
    fi
    auths="Bearer ${auth}"
    case "$auth" in
        FlyV1\ *) ;;
        *) auths="$auths
FlyV1 ${auth}" ;;
    esac
    i=0
    printf '%s\n' $ids | while read -r cid; do
        [ -n "$cid" ] || continue
        for base in https://api.fly.io https://fly.io; do
            for pathpfx in postgresv2 postgres; do
                printf '%s\n' "$auths" | while IFS= read -r ah; do
                    i=$((i + 1))
                    url="${base}/api/v1/${pathpfx}/${cid}${path}"
                    code=$(curl -sS -o "$work/api.$i" -w '%{http_code}' \
                        -X POST \
                        -H "Authorization: ${ah}" \
                        -H "Content-Type: application/json" \
                        -d "$body" \
                        "$url" || echo 000)
                    echo "POST /api/v1/${pathpfx}/…${path} -> HTTP $code" >&2
                    python3 - "$work/api.$i" <<'PY' 2>/dev/null || true
import json, sys
from pathlib import Path
p = Path(sys.argv[1])
if not p.exists() or p.stat().st_size == 0:
    raise SystemExit(0)
try:
    d = json.loads(p.read_text())
except Exception:
    raise SystemExit(0)
if not isinstance(d, dict):
    raise SystemExit(0)
if any(k.lower() == "password" for k in d.keys()) or "password" in json.dumps(d).lower():
    raise SystemExit(0)
msg = d.get("error") or d.get("message") or (d.get("errors") or [None])[0]
if msg:
    print(str(msg)[:200], file=sys.stderr)
PY
                    rm -f "$work/api.$i"
                    case "$code" in
                        200|201|204|409) exit 0 ;;
                    esac
                done
            done
        done
    done
    echo "MPG API POST ${path} failed." >&2
    return 1
}

if ok_or_exists "Database earth" mpg databases create "$CLUSTER" -n earth; then
    :
elif mpg_post "/databases" '{"name":"earth"}'; then
    echo "Database earth created via API."
else
    exit 1
fi

if ok_or_exists "User earth" mpg users create "$CLUSTER" -u earth -r writer; then
    :
elif mpg_post "/users" '{"user_name":"earth","role":"writer"}'; then
    echo "User earth created via API."
else
    exit 1
fi

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
