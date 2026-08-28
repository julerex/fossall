#!/bin/sh
# One-time: database `earth` + user `earth` on postgreputest.
# Does not print connection URLs. Do not `fly mpg attach` (that overwrites DATABASE_URL).
set -eu
CLUSTER="${MPG_CLUSTER:-q49ypo4wvmzr17ln}"
ROOT="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"

if ! command -v fly >/dev/null 2>&1 && ! command -v flyctl >/dev/null 2>&1; then
  echo "fly/flyctl is required" >&2
  exit 1
fi
FLY="$(command -v fly || true)"
if [ -z "$FLY" ]; then
  FLY="$(command -v flyctl)"
fi

"$FLY" mpg databases create "$CLUSTER" -n earth
"$FLY" mpg users create "$CLUSTER" -u earth -r writer
"$FLY" mpg connect "$CLUSTER" -d earth < "$ROOT/db/earth/001_init.sql"

echo "Schema applied to database earth."
echo "Next: fly mpg proxy $CLUSTER"
echo "Then set EARTH_DATABASE_URL to the earth database on localhost (never commit it)."
echo "Then: make seed-earth"
echo "Then: fly secrets set EARTH_DATABASE_URL=... -a fossall"
echo "Do not run fly mpg attach for this database."
