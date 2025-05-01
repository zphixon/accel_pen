FROM postgres:17.4-bookworm
COPY schema.sql /docker-entrypoint-initdb.d