# The tools the ingests beside this folder run, so none of them is the
# host's: GDAL's warp and vector copy, PostGIS's raster loader, the
# PostgreSQL client, and Overture's CLI. Ubuntu 26.04 ships PostgreSQL 18,
# db-geo's major, and PostGIS 3.6, one release behind db-geo's 3.7, which no
# Ubuntu release ships; within the release its packages take patches, never a
# new upstream version. `run` builds it and runs a tool in it.
FROM ubuntu:26.04@sha256:065aa8626841f16057b78b027b464cd59c4fac36c204c72dcbf48d66f5fa8a96

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        gdal-bin postgis postgresql-client python3-venv ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The CLI's release the Overture ingest was written against: a new one is
# taken by changing this, not by whatever pip finds that day, so a change in
# the CLI's behavior arrives with a commit rather than unnoticed.
RUN python3 -m venv /opt/overturemaps \
    && /opt/overturemaps/bin/pip install --no-cache-dir overturemaps==2.1.1
ENV PATH=/opt/overturemaps/bin:$PATH
