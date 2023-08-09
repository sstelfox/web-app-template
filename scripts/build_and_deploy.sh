#!/usr/bin/env bash

LOCAL_CACHE_MAX=5
ARCHIVE_MAX=15

REPO="repo.stelfox.net"
IMAGE="web-app-template"

# todo: tag with git describe
TAG="latest"

REMOTE_HOST="hollow-twilight-ocean.stelfox.net"
REMOTE_IMAGE_ARCHIVE="/srv/container_archive"

IMAGE_BASE_NAME="${REPO}_${IMAGE}_${TAG}.$(date +%Y%m%d%H%M%S)"

podman build -t $REPO/$IMAGE:$TAG ./
podman save -o target/${IMAGE_BASE_NAME}.tar $REPO/$IMAGE:$TAG

scp target/${IMAGE_BASE_NAME}.tar ${REMOTE_HOST}:${REMOTE_IMAGE_ARCHIVE}

# On the remote system:
# ```sh
# podman load < ${REMOTE_IMAGE_ARCHIVE}/${IMAGE_BASE_NAME}.tar
#
# # This doesn't work.. json format is wrong
# for image in $(podman images $REPO/$IMAGE --format json | jq -r .RepoTags[0] | tail -n +\$((LOCAL_CACHE_MAX+1))\`; do
#   podman rmi $image
# done
#
# ls -ltr ${REMOTE_IMAGE_ARCHIVE}/${REPO}_${IMAGE}*.tar | head -n -${ARCHIVE_MAX} | xargs --no-run-if-empty rm
# ```


