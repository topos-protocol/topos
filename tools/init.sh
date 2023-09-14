#!/bin/bash

set -e

if ! command -v jq &> /dev/null
then
    echo "jq could not be found"
    exit
fi

JQ=$(which jq)
TOPOS_BIN=./topos
BOOT_PEERS_PATH=/tmp/shared/boot_peers.json
PEER_LIST_PATH=/tmp/shared/peer_ids.json
NODE_LIST_PATH=/tmp/shared/peer_nodes.json
NODE="http://$HOSTNAME:1340"
TCE_EXT_HOST="/dns4/$HOSTNAME"

case "$1" in

    "boot")
        BOOT_PEER_ID=$($TOPOS_BIN tce keys --from-seed=$HOSTNAME)

        echo "Generating boot_peers file..."
        $JQ -n --arg PEER $BOOT_PEER_ID --arg ADDR $TCE_EXT_HOST '[$PEER + " " + $ADDR + "/tcp/9090"]' > $BOOT_PEERS_PATH
        echo "Boot peers list file have been successfully generated"

        echo "Generating peer list file..."
        $JQ -n --arg PEER $BOOT_PEER_ID '[$PEER]' > $PEER_LIST_PATH
        echo "Peer list file have been successfully generated"

        cat $PEER_LIST_PATH

        echo "Generating node list file..."
        $JQ -n --arg NODE $NODE '{"nodes": [$NODE]}' > $NODE_LIST_PATH
        echo "Peer nodes list have been successfully generated"

        echo "Starting boot node..."

        echo "BOOT_PEER_ID: $BOOT_PEER_ID"
        echo "TCE_LOCAL_KS: $HOSTNAME"

        export TCE_LOCAL_KS=$HOSTNAME
        export TCE_LOCAL_VPK=122f3ae6ade1fd136b292cea4f6243c7811160352c8821528547a1fe7c459daf
        export TCE_VALIDATORS="0xfa1dbe573e7ab7eb4fb772a349129cf0833f1154"
        export TCE_EXT_HOST

        exec "$TOPOS_BIN" "${@:2}"
        ;;

   "peer")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then

           until [ -f "$BOOT_PEERS_PATH" ]
           do
               echo "Waiting 1s for boot_peers file $BOOT_PEERS_PATH to be created by boot container..."
               sleep 1
           done

           export TCE_BOOT_PEERS=$(cat $BOOT_PEERS_PATH | $JQ -r '.|join(",")')

           until [ -f "$PEER_LIST_PATH" ]
           do
               echo "Waiting 1s for peer_list file $PEER_LIST_PATH to be created by boot container..."
               sleep 1
           done

           PEER=$($TOPOS_BIN tce keys --from-seed=$HOSTNAME)

           # Acquire lock and add $PEER to ${PEER_LIST_PATH} only once
           (
               flock --exclusive -w 10 201 || exit 1
               cat <<< $($JQ --arg PEER $PEER '. |= (. + [$PEER] | unique )' $PEER_LIST_PATH) > $PEER_LIST_PATH

           ) 201>"${PEER_LIST_PATH}.lock"

           export TCE_LOCAL_KS=$HOSTNAME
           export TCE_LOCAL_VPK=a2e33a9bad88f7b7568228f51d5274c471a9217162d46f1533b6a290f0be1baf
           export TCE_VALIDATORS="0xfa1dbe573e7ab7eb4fb772a349129cf0833f1154"
           export TCE_EXT_HOST

           until [ -f "$NODE_LIST_PATH" ]
           do
               echo "Waiting 1s for node_list file $NODE_LIST_PATH to be created by boot container..."
               sleep 1
           done

           # Acquire lock and add $NODE to ${NODE_LIST_PATH} only once
           (
               flock --exclusive -w 10 200 || exit 1
               cat <<< $($JQ --arg NODE $NODE '.nodes |= (. + [$NODE] | unique)' $NODE_LIST_PATH) > $NODE_LIST_PATH

           ) 200>"${NODE_LIST_PATH}.lock"

           cat $PEER_LIST_PATH

           echo "PEER_ID: $PEER"
           echo "TCE_LOCAL_KS: $HOSTNAME"

       fi

       exec "$TOPOS_BIN" "${@:2}"
       ;;

   *)
       exec "$TOPOS_BIN" "$@"
       ;;
esac
