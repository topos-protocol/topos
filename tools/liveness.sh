#! /bin/bash

set -u -o pipefail

if [ "$#" -ne 1 ]; then
    printf "USAGE: ./liveness.sh [boot|peer]"
fi

case "$1" in

    "boot")
            topos tce status --endpoint http://localhost:1340;
        ;;

   *)
            topos tce status --endpoint http://localhost:1340;
       ;;

esac
