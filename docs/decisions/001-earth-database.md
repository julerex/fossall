# ADR-001: Separate `earth` database and CC-BY paleogeography

## Status
Accepted

## Date
2026-08-28

## Context
Fossall already has a five-letter word list in database `fossall`, schema `words`, on Fly Managed Postgres cluster **postgreputest**. `/earth` needs geologic time, taxonomy, fossil occurrences, lithology, and reconstructed continent polygons. Those data are large relative to the word list, come from different licenses, and should not share a schema with ENABLE words.

Paleogeographic maps that look like “what Earth looked like” are mostly copyrighted (Blakey, much of Scotese PaleoAtlas). Public-domain NASA Blue Marble is present-day satellite imagery and is the wrong picture for deep time. The Fly app is a 256 MB Machine and must not call heavy reconstruction services at request time.

## Decision
1. Create a **second database named `earth`** on the existing **postgreputest** cluster (new user `earth`). Do not create a second MPG cluster. Do not `fly mpg attach` it; the app uses `EARTH_DATABASE_URL` so the words `DATABASE_URL` stays intact.
2. Store a highly normalized schema in `earth` (sources, time, taxa, collections, occurrences, lithology, stratigraphy, reconstructions). Geometries are JSONB GeoJSON so PostGIS is not required.
3. Use **CC-BY** sources (ICS, Macrostrat, PBDB, GPlates/CAO2024) with on-page attribution, not copyrighted atlases and not a claim of public domain.
4. Precompute CAO2024 coastline snapshots offline (`seed-earth recon`) every 10 Ma from 0–1800 Ma. Runtime only reads Postgres.
5. Keep Three.js as a page-local ES module (same CDN import map as `/rv`), not an SPA.

## Alternatives considered

### Same `fossall` database, new schema
Would mix unrelated products and make restore/seed harder. Rejected: the request was a new database, and isolation is cheap on MPG.

### New Fly Postgres cluster
Rejected: cost, and AGENTS.md forbids a second cluster.

### Call GPlates Web Service from the Fly app
Rejected: latency, dependency, and 256 MB Machines. Snapshots are ingested once.

### Photorealistic Earth textures
Rejected: anachronistic for deep time; large assets.

### Catalogue of Life + GBIF in v1
Rejected for size on the shared cluster. Fossil-first (PBDB + Macrostrat + ICS) matches the education goal.

## Consequences
- Operators must set `EARTH_DATABASE_URL` locally and as a Fly secret, separately from `DATABASE_URL`.
- Seed jobs need network access to Macrostrat, PBDB, and GWS and can take a long time for the full PBDB dump.
- Every public page must keep CC-BY citations visible.
