#! /bin/bash

set -u -o pipefail

if [ "$#" -ne 1 ]; then
    printf "USAGE: ./liveness.sh [boot|peer]"
fi

case "$1" in

    "boot")
            topos tce push-peer-list --endpoint http://localhost:1340 --format json /tmp/shared/peer_ids.json;
        ;;

   *)
            topos tce push-peer-list --endpoint http://localhost:1340 --format json /tmp/shared/peer_ids.json && topos tce status --endpoint http://localhost:1340;
       ;;

esac
