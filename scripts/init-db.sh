#!/bin/bash
set -e

# DB has two components - Postgres and Flightsql-pg
# This script would start Postgres on local container and then start flightsql

# Initialize the database
if [ ! -s /pg_data/PG_VERSION ]; then
    echo "Initializing PostgreSQL database..."
    /usr/lib/postgresql/14/bin/pg_ctl initdb -D /pg_data
    echo "Database initialized."
fi

# Start PostgreSQL in the background
(/usr/lib/postgresql/14/bin/pg_ctl start -D /pg_data | rotatelogs /logs/postgres/postgresql.log 86400 &) && \
sleep 3 && psql -U postgres -d postgres -c "select * from pg_roles where rolname = 'postgres'" -q|grep " postgres" || /usr/lib/postgresql/14/bin/createuser -s postgres

# Create Metastore tables
psql -U postgres -d postgres -a -f /opt/metadata.sql

# Start flightsql-pg in the background
source /opt/start-flightsql.sh
