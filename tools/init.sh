#!/bin/bash

set -e

if ! command -v jq &> /dev/null
then
    echo "jq could not be found"
    exit
fi

JQ=$(which jq)
TOPOS_BIN=./topos
TOPOS_HOME=/tmp/node_config
PEER_LIST_PATH=/tmp/shared/peer_ids.json
NODE_LIST_PATH=/tmp/shared/peer_nodes.json
NODE="http://$HOSTNAME:1340"
TCE_EXT_HOST="/dns4/$HOSTNAME"
FIXED_BOOT_PEER_ID="BOOT_NODE_1"

case "$1" in

    "boot")
        BOOT_PEER_ID=$($TOPOS_BIN tce keys --from-seed=$FIXED_BOOT_PEER_ID)

        echo "Generating peer list file..."
        $JQ -n --arg PEER $BOOT_PEER_ID '[$PEER]' > $PEER_LIST_PATH
        echo "Peer list file have been successfully generated"

        cat $PEER_LIST_PATH

        echo "Generating node list file..."
        $JQ -n --arg NODE $NODE '{"nodes": [$NODE]}' > $NODE_LIST_PATH
        echo "Peer nodes list have been successfully generated"

        echo "Starting boot node..."

        echo "BOOT_PEER_ID: $BOOT_PEER_ID"
        echo "TCE_LOCAL_KS: b2d9e427c3be"

        export TOPOS_HOME=$TOPOS_HOME
        export TCE_LOCAL_KS="b2d9e427c3be"
        export TCE_EXT_HOST="/dns4/b2d9e427c3be"

        if [ ! -d "/tmp/boot_node_keys/" ]; then
            echo "Directory /tmp/boot_node_keys/ does not exist!"
        fi

        if [ ! -w "$TOPOS_HOME/node/test/libp2p/" ]; then
            echo "Directory $TOPOS_HOME/node/test/libp2p/ is not writable!"
        fi

        LIBP2P_KEY=$(cat /tmp/boot_node_keys/libp2p.key)
        echo -n "$LIBP2P_KEY" > "$TOPOS_HOME/node/test/libp2p/libp2p.key"

        VALIDATOR_BLS_KEY=$(cat /tmp/boot_node_keys/validator-bls.key)
        echo -n "$VALIDATOR_BLS_KEY" > $TOPOS_HOME/node/test/consensus/validator-bls.key

        $VALIDATOR_KEY=$(cat /tmp/boot_node_keys/validator.key)
        echo -n "$VALIDATOR_KEY" > $TOPOS_HOME/node/test/consensus/validator.key

        exec "$TOPOS_BIN" "${@:2}"
        ;;

   "peer")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then

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

           export TOPOS_HOME=$TOPOS_HOME
           export TCE_LOCAL_KS=$HOSTNAME
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

          (
            flock -x 200
            LIBP2P_KEY=$(jq -r ".keys[0]" /tmp/shared/libp2p_keys.json)
            echo -n "$LIBP2P_KEY" > $TOPOS_HOME/node/test/libp2p/libp2p.key
            jq "del(.keys[0])" /tmp/shared/libp2p_keys.json > /tmp/shared/libp2p_keys_temp.json && cat /tmp/shared/libp2p_keys_temp.json > /tmp/shared/libp2p_keys.json && rm /tmp/shared/libp2p_keys_temp.json
          ) 200>/tmp/shared/libp2p_keys.lock

          (
            flock -x 201
            VALIDATOR_BLS_KEY=$(jq -r ".keys[0]" /tmp/shared/validator_bls_keys.json)
            echo -n "$VALIDATOR_BLS_KEY" > $TOPOS_HOME/node/test/consensus/validator-bls.key
            jq "del(.keys[0])" /tmp/shared/validator_bls_keys.json > /tmp/shared/validator_bls_keys_temp.json && cat /tmp/shared/validator_bls_keys_temp.json > /tmp/shared/validator_bls_keys.json && rm /tmp/shared/validator_bls_keys_temp.json
          ) 201>/tmp/shared/validator_bls_keys.lock

          (
            flock -x 202
            VALIDATOR_KEY=$(jq -r ".keys[0]" /tmp/shared/validator_keys.json)
            echo -n "$VALIDATOR_KEY" > $TOPOS_HOME/node/test/consensus/validator.key
            jq "del(.keys[0])" /tmp/shared/validator_keys.json > /tmp/shared/validator_keys_temp.json && cat /tmp/shared/validator_keys_temp.json > /tmp/shared/validator_keys.json && rm /tmp/shared/validator_keys_temp.json
          ) 202>/tmp/shared/validator_keys.lock

       fi

       exec "$TOPOS_BIN" "${@:2}"
       ;;

   "sync")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then

           until [ -f "$PEER_LIST_PATH" ]
           do
               echo "Waiting 1s for peer_list file $PEER_LIST_PATH to be created by boot container..."
               sleep 1
           done

           export TCE_LOCAL_KS=$HOSTNAME
           export TCE_EXT_HOST

           until [ -f "$NODE_LIST_PATH" ]
           do
               echo "Waiting 1s for node_list file $NODE_LIST_PATH to be created by boot container..."
               sleep 1
           done

           echo "TCE_LOCAL_KS: $HOSTNAME"

           # For libp2p_keys.json
           LIBP2P_KEY=$(jq -r ".keys[0]" /tmp/shared/libp2p_keys.json)
           echo -n "$LIBP2P_KEY" > $TOPOS_HOME/node/test/libp2p/libp2p.key
           jq "del(.keys[0])" /tmp/shared/libp2p_keys.json > /tmp/shared/libp2p_keys_temp.json && mv /tmp/shared/libp2p_keys_temp.json /tmp/shared/libp2p_keys.json

           # For validator_bls_keys.json
           VALIDATOR_BLS_KEY=$(jq -r ".keys[0]" /tmp/shared/validator_bls_keys.json)
           echo -n "$VALIDATOR_BLS_KEY" > $TOPOS_HOME/node/test/consensus/validator-bls.key
           jq "del(.keys[0])" /tmp/shared/validator_bls_keys.json > /tmp/shared/validator_bls_keys_temp.json && mv /tmp/shared/validator_bls_keys_temp.json /tmp/shared/validator_bls_keys.json

           # For validator_keys.json
           VALIDATOR_KEY=$(jq -r ".keys[0]" /tmp/shared/validator_keys.json)
           echo -n "$VALIDATOR_KEY" > $TOPOS_HOME/node/test/consensus/validator.key
           jq "del(.keys[0])" /tmp/shared/validator_keys.json > /tmp/shared/validator_keys_temp.json && mv /tmp/shared/validator_keys_temp.json /tmp/shared/validator_keys.json

       fi

       exec "$TOPOS_BIN" "${@:2}"
       ;;


   *)
       exec "$TOPOS_BIN" "$@"
       ;;
esac
