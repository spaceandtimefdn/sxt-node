#!/bin/bash
# This is a basic script to get the node up
# Script calls init-db to start database and then starts sxt-node
# TO DO :
# 1. Checks for startup of upstream services
# 2. Check for Node start up arguments
# 3. If need be, add conditions for start up arguments

# Start the DB initialization script
echo "Running Postgres initialization script"
/opt/init-db.sh

# Start sxt-node in the background
echo "Starting sxt-node with $@"

/usr/local/bin/sxt-node "$@"

