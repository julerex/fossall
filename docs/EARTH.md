# Deep-time Earth (`/earth`)

Educational globe of reconstructed continents, fossil taxa, and rock/sediment vocabularies. It is **not** Google Earth imagery or a complete catalog of every species.

Page: `https://fossall.com/earth` (or `http://localhost:8080/earth`). JSON under `/api/earth/*`. If `EARTH_DATABASE_URL` is unset the globe still renders; data routes return 503.

## Sources (CC-BY, not public domain)

High-quality paleogeographic atlases (Blakey Deep Time Maps, most Scotese PaleoAtlas products) are copyrighted. v1 uses reusable CC-BY data and attributes it on the page:

| Layer | Source | License | Notes |
|---|---|---|---|
| Continents | GPlates Web Service model **CAO2024** (Cao et al. 2024) | CC-BY | Full-plate model **0–1800 Ma**. Snapshots stored locally; the Fly app does **not** call GWS at runtime. |
| Geologic time | ICS chart via Macrostrat `defs/intervals?timescale=international intervals` | CC-BY 4.0 | Hadean (4567–4031 Ma) is inserted as the informal ICS eon if missing. |
| Fossils | [Paleobiology Database](https://paleobiodb.org/data1.2/) taxa, collections, occurrences | CC-BY 4.0 | Paleocoordinates come from PBDB (`paleolat` / `paleolng`), not a live GPlates call. |
| Lithology, environments, stratigraphic names | Macrostrat API v2 `defs/*` | CC-BY 4.0 | Vocabularies only — not the 225-map polygon dump. |

v1 does **not** ingest Catalogue of Life, GBIF occurrences, or Macrostrat map tiles.

## Database

New database `earth` on cluster **postgreputest** (not a second cluster). Schema: [`db/earth/001_init.sql`](../db/earth/001_init.sql). App user `earth`. Connection: **`EARTH_DATABASE_URL`**. Do **not** `fly mpg attach` this database (that would overwrite the words `DATABASE_URL`).

Setup and seed: [DATABASE.md](DATABASE.md).

## API

| Path | |
|---|---|
| `GET /api/earth/timescale` | ICS units (rank, ages, chart colors) |
| `GET /api/earth/continents?ma=` | Nearest stored reconstruction as a GeoJSON FeatureCollection |
| `GET /api/earth/taxa?q=` | Prefix search, max 20 |
| `GET /api/earth/occurrences?ma=&taxon_id=&limit=` | Paleocoordinate points, hard cap 2000 |

`/health` never queries Postgres.

## Seed

Dumps are gitignored under `data/earth/`. The seeder talks to Macrostrat, PBDB, and GWS with User-Agent `fossall-earth-seed` via `curl`.

```bash
export EARTH_DATABASE_URL='postgres://…@localhost:16380/earth'
make seed-earth              # vocab + pbdb + recon
make seed-earth-vocab
make seed-earth-pbdb
make seed-earth-recon
```

Optional env: `EARTH_PBDB_BASE_NAME` (e.g. `Dinosauria` instead of all records), `EARTH_PBDB_MAX_TAXA` / `EARTH_PBDB_MAX_COLLS` / `EARTH_PBDB_MAX_OCCS`, `EARTH_RECON_STRIDE` (default 10 Ma), `EARTH_RECON_MAX_MA` (default 1800).

A full PBDB ingest is hundreds of MB and many minutes. Reconstruction ingest is ~181 GWS requests (0–1800 Ma every 10 Ma) with simplified polygons.

## UI

Three.js (CDN, same import map as `/rv`) in [`static/js/earth-globe.js`](../static/js/earth-globe.js). The `/earth` nav link sets `hx-boost="false"` so the canvas mounts on a full page load. The globe never downloads all taxa: each slider tick fetches one continent snapshot and a capped occurrence sample.
