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

# I'm going to use systemd user units to manage podman instances in the user
# namespace but hosting on the host itself using configurable ports.
#
# To make sure the processes run after my SSH session logout I need to enable
# linger on my user by running the following command as root:
#
# ```
# loginctl enable-linger sstelfox
# ```
#
# Sample user systemd instance:
#
# ```
# ~/.config/systemd/user/mpd.service
#
# [Unit]
# Description=Music Player Daemon
#
# [Service]
# ExecStart=/usr/bin/mpd --no-daemon
#
# [Install]
# WantedBy=default.target
# ```
#
# Generate podman container definitions:
#
#
# ```
# $ podman pod create --name=my-pod
# 635bcc5bb5aa0a45af4c2f5a508ebd6a02b93e69324197a06d02a12873b6d1f7
#
# $ podman create --pod=my-pod --name=container-a -t centos top
# c04be9c4ac1c93473499571f3c2ad74deb3e0c14f4f00e89c7be3643368daf0e
#
# $ podman create --pod=my-pod --name=container-b -t centos top
# b42314b2deff99f5877e76058ac315b97cfb8dc40ed02f9b1b87f21a0cf2fbff
#
# $ cd $HOME/.config/systemd/user
#
# $ podman generate systemd --new --files --name my-pod
# /home/vrothberg/.config/systemd/user/pod-my-pod.service
# /home/vrothberg/.config/systemd/user/container-container-b.service
# /home/vrothberg/.config/systemd/user/container-container-a.service
# ```
#
# Other relevant snippet:
#
# ```
# That is all you need to know about generating systemd units for pods with Podman. Once you've reloaded systemd via systemctl --user daemon-reload, start and stop the pod.service at will. Have a look:
# ```
#
# Deploy script:
#
#!/usr/bin/env bash

set -e
cd $(dirname $0)

if [ "$#" -ne 2 ]; then
	echo "usage: $0 user@server-address /path/to/remote/directory/"
	exit 1
fi

SERVER_SSH=$1
SERVER_PATH=$2
BINARY_NAME="example"
SERVER_RESTART_COMMAND="systemctl restart $BINARY_NAME"

./build.sh

OUTFILE="./target/x86_64-unknown-linux-musl/release/$BINARY_NAME"
COMMIT_HASH=$(git rev-parse HEAD)
BUILD_TIMESTAMP=$(TZ=UTC date -u +"%s")
FILE_HASH=$(b2sum $OUTFILE | cut -f1 -d' ')
REMOTE_FILENAME="$BINARY_NAME-$BUILD_TIMESTAMP-$COMMIT_HASH-$FILE_HASH"

ssh $SERVER_SSH "mkdir -p $SERVER_PATH/versions/"
scp "$OUTFILE" "$SERVER_SSH:$SERVER_PATH/versions/$REMOTE_FILENAME"
ssh -q -T $SERVER_SSH <<EOL
	nohup sh -c "\
	rm "$SERVER_PATH/$BINARY_NAME" && \
	ln -s "$SERVER_PATH/versions/$REMOTE_FILENAME" "$SERVER_PATH/$BINARY_NAME" && \
	$SERVER_RESTART_COMMAND"
EOL
