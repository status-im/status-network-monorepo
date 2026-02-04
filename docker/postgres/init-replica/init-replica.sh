#!/bin/bash
set -e

# Wait for primary to be ready
until PGPASSWORD=$POSTGRES_PASSWORD psql -h postgres -U $POSTGRES_USER -d prover_db -c '\q'; do
  >&2 echo "Primary Postgres is unavailable - sleeping"
  sleep 1
done

>&2 echo "Primary Postgres is up"

# Logic to ADD tables to publication after they are created by the application (rln-prover)
# We loop until the tables exist on the Primary
MAX_RETRIES=30
for i in $(seq 1 $MAX_RETRIES); do
    if PGPASSWORD=$POSTGRES_PASSWORD psql -h postgres -U $POSTGRES_USER -d prover_db -c "SELECT 'tx_counter'::regclass;" >/dev/null 2>&1; then
        echo "Table tx_counter found on Primary. Adding to publication..."
        PGPASSWORD=$POSTGRES_PASSWORD psql -h postgres -U $POSTGRES_USER -d prover_db -c "ALTER PUBLICATION rln_publication ADD TABLE tx_counter, deny_list;" || true
        break
    else
        echo "Waiting for rln-prover to create tables on Primary... ($i/$MAX_RETRIES)"
        sleep 2
    fi
done

# Create Database if it doesn't exist (it enters here as default postgres db)
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    SELECT 'CREATE DATABASE prover_db'
    WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'prover_db')\gexec
EOSQL

# Create Schema in prover_db
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "prover_db" <<-EOSQL
    CREATE TABLE IF NOT EXISTS tx_counter (
        id BIGSERIAL NOT NULL PRIMARY KEY,
        address TEXT UNIQUE,
        epoch BIGINT DEFAULT 0,
        epoch_counter BIGINT DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS deny_list (
        address CHAR(42) NOT NULL PRIMARY KEY,
        expires_at BIGINT,
        denied_at BIGINT
    );

    CREATE SUBSCRIPTION rln_subscription
        CONNECTION 'host=postgres port=5432 user=postgres password=postgres dbname=prover_db'
        PUBLICATION rln_publication;
EOSQL
