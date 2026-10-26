# The tools the ingests beside this folder run, so none of them is the
# host's: GDAL's warp and vector copy, PostGIS's raster loader, the
# PostgreSQL client, and Overture's CLI. Ubuntu 24.04 ships PostgreSQL 16 and
# PostGIS 3.4, db-geo's majors, so what fills the database is the version it
# is; within the release its packages take patches, never a new upstream
# version. `run` builds it and runs a tool in it.
FROM ubuntu:24.04@sha256:68ee22004a9109b44973313b02ff7b1de250f1de0f5ecb628d73f579aaff33ac

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        gdal-bin postgis postgresql-client python3-venv ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# The CLI's release the Overture ingest was written against: a new one is
# taken by changing this, not by whatever pip finds that day, so a change in
# the CLI's behaviour arrives with a commit rather than unnoticed.
RUN python3 -m venv /opt/overturemaps \
    && /opt/overturemaps/bin/pip install --no-cache-dir overturemaps==1.0.2
ENV PATH=/opt/overturemaps/bin:$PATH
