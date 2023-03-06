#!/bin/bash

set -e

if ! command -v jq &> /dev/null
then
    echo "jq could not be found"
    exit
fi

JQ=$(which jq)
TOPOS_BIN=./topos
TMP_PEER_LIST_PATH=/tmp/shared/peer_ids.tmp
TMP_NODE_LIST_PATH=/tmp/shared/peer_nodes.tmp
BOOT_PEERS_PATH=/tmp/shared/boot_peers.json
PEER_LIST_PATH=/tmp/shared/peer_ids.json
NODE_LIST_PATH=/tmp/shared/peer_nodes.json
NODE="http://$HOSTNAME:1340"
TCE_EXT_HOST="/dns4/$HOSTNAME"

case "$1" in

    "boot")
        BOOT_PEER_ID=$($TOPOS_BIN tce keys --from-seed=$TCE_LOCAL_KS)

        # clean up shared directory
        rm $BOOT_PEERS_PATH $PEER_LIST_PATH $NODE_LIST_PATH 2> /dev/null || echo "everything's clean!"

        echo "Generating boot_peers file..."
        $JQ -n --arg PEER $BOOT_PEER_ID --arg ADDR $TCE_EXT_HOST '[$PEER + " " + $ADDR + "/tcp/9090"]' > $BOOT_PEERS_PATH
        echo "Boot peers list file have been successfully generated"

        echo "Generating peer list file..."
        $JQ -n --arg PEER $BOOT_PEER_ID '[$PEER]' > $PEER_LIST_PATH
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

           if ! grep -q $PEER $PEER_LIST_PATH; then
               $JQ --arg PEER $PEER '. += [$PEER]' $PEER_LIST_PATH > "${TMP_PEER_LIST_PATH}" \
                && mv "${TMP_PEER_LIST_PATH}" $PEER_LIST_PATH
           fi

           export TCE_LOCAL_KS=$HOSTNAME
           export TCE_EXT_HOST

           until [ -f "$NODE_LIST_PATH" ]
           do
               echo "Waiting 1s for node_list file $NODE_LIST_PATH to be created by boot container..."
               sleep 1
           done

           if ! grep -q $NODE $NODE_LIST_PATH; then
               $JQ --arg NODE $NODE '.nodes += [$NODE]' $NODE_LIST_PATH > "${TMP_NODE_LIST_PATH}" \
                && mv "${TMP_NODE_LIST_PATH}" $NODE_LIST_PATH
           fi
       fi

       exec "$TOPOS_BIN" "$@"
       ;;

esac
