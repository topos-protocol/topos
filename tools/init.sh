#!/bin/bash

set -e

if ! command -v jq &> /dev/null
then
    echo "jq could not be found"
    exit
fi

JQ=$(which jq)
TOPOS_BIN=./topos
PEER_LIST_PATH=/tmp/shared/peer_ids.json
NODE_LIST_PATH=/tmp/shared/peer_nodes.json
NODE="http://$HOSTNAME:1340"
TCE_EXT_HOST="/dns4/$HOSTNAME"

case "$1" in

    "boot")
        echo "Generating peer list file..."
        $JQ -n --arg PEER $($TOPOS_BIN tce peer-id --from-slice=$TCE_LOCAL_KS) '[$PEER]' > $PEER_LIST_PATH
        echo "Peer list file have been successfully generated"

        echo "Generating node list file..."
        $JQ -n --arg NODE $NODE '{"nodes": [$NODE]}' > $NODE_LIST_PATH
        echo "Peer nodes list have been successfully generated"

        echo "Starting boot node..."

        export TCE_EXT_HOST

        exec "$TOPOS_BIN" "${@:2}"
        ;;

   *)
       if [[ ! -z ${LOCAL_TEST_NET+x} ]]; then
           until [ -f "$PEER_LIST_PATH" ]
           do
               echo "Waiting 1s for peer_list file $PEER_LIST_PATH to be created by boot container..."
               sleep 1
           done

           PEER=$($TOPOS_BIN tce peer-id --from-slice=$HOSTNAME)
           cat <<< $($JQ --arg PEER $PEER '. += [$PEER]' $PEER_LIST_PATH) > $PEER_LIST_PATH

           export TCE_LOCAL_KS=$HOSTNAME
           export TCE_EXT_HOST

           until [ -f "$NODE_LIST_PATH" ]
           do
               echo "Waiting 1s for node_list file $NODE_LIST_PATH to be created by boot container..."
               sleep 1
           done

           cat <<< $($JQ --arg NODE $NODE '.nodes += [$NODE]' $NODE_LIST_PATH) > $NODE_LIST_PATH
       fi

       exec "$TOPOS_BIN" "$@"
       ;;

esac
