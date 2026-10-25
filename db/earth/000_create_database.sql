-- Create the earth database on the shared MPG cluster.
-- Safe to re-run: skip if earth already exists.
SELECT 'CREATE DATABASE earth'
WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'earth')\gexec
