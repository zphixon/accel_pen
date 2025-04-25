FROM mysql:latest
WORKDIR /app
COPY schema.sql /docker-entrypoint-initdb.d
COPY migrations .