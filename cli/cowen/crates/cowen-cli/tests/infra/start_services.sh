#!/bin/bash
# In-container Database Services Startup Script (System-init optimized)

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}[CONTAINER-DB] Starting in-container database services...${NC}"

# --- 1. Redis ---
echo -n "  Starting Redis..."
/etc/init.d/redis-server start > /dev/null 2>&1
for i in {1..10}; do
    if redis-cli ping > /dev/null 2>&1; then
        echo -e " ${GREEN}[OK]${NC}"
        break
    fi
    sleep 0.5
done

# --- 2. MySQL ---
echo -n "  Starting MySQL..."
# Fix for potential MySQL directory permission issue in rootless container
mkdir -p /run/mysqld && chown -R mysql:mysql /run/mysqld /var/lib/mysql

/etc/init.d/mysql start > /tmp/mysql_start.log 2>&1
for i in {1..30}; do
    if mysqladmin ping --silent > /dev/null 2>&1; then
        echo -e " ${GREEN}[OK]${NC}"
        mysql -e "CREATE DATABASE IF NOT EXISTS cowen_test;" > /dev/null 2>&1 || true
        break
    fi
    sleep 1
done

# --- 3. PostgreSQL ---
echo -n "  Starting PostgreSQL..."
PG_CONF_DIR=$(ls -d /etc/postgresql/*/main | head -n 1)
if [ -d "$PG_CONF_DIR" ]; then
    # Force trust authentication for all local connections
    echo "local all all trust" > "$PG_CONF_DIR/pg_hba.conf"
    echo "host all all 127.0.0.1/32 trust" >> "$PG_CONF_DIR/pg_hba.conf"
    echo "host all all ::1/128 trust" >> "$PG_CONF_DIR/pg_hba.conf"
fi

# Ensure the socket directory exists and has correct permissions
mkdir -p /var/run/postgresql && chown postgres:postgres /var/run/postgresql

/etc/init.d/postgresql start > /tmp/pg_start.log 2>&1
for i in {1..30}; do
    if su - postgres -c "pg_isready" > /dev/null 2>&1; then
        echo -e " ${GREEN}[OK]${NC}"
        su - postgres -c "psql -d postgres -c 'CREATE DATABASE IF NOT EXISTS cowen_test;'" > /dev/null 2>&1 || true
        break
    fi
    sleep 1
done

# --- Final Check ---
READY=0
redis-cli ping > /dev/null 2>&1 && READY=$((READY + 1))
mysqladmin ping --silent > /dev/null 2>&1 && READY=$((READY + 1))
su - postgres -c "pg_isready" > /dev/null 2>&1 && READY=$((READY + 1))

if [ "$READY" -ge 3 ]; then
    echo -e "${GREEN}[CONTAINER-DB] ✅ All 3 database services are running on 127.0.0.1${NC}"
    export DB_HOST="127.0.0.1"
    export DB_HOST_DETECTED="true"
else
    echo -e "${RED}[CONTAINER-DB] ❌ Only $READY/3 services started.${NC}"
    echo -e "${YELLOW}--- MySQL Start Log ---${NC}"
    cat /tmp/mysql_start.log 2>/dev/null
    echo -e "${YELLOW}--- PostgreSQL Start Log ---${NC}"
    cat /tmp/pg_start.log 2>/dev/null
    exit 1
fi
