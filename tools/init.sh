#!/bin/bash

set -e

if ! command -v jq &> /dev/null
then
    echo "jq could not be found"
    exit
fi

JQ=$(which jq)
TOPOS_BIN=/usr/src/app/topos
TOPOS_HOME=/tmp/node_config
NODE="http://$HOSTNAME:1340"
TCE_EXT_HOST="/dns4/$HOSTNAME"
NODE_LIST_PATH=/tmp/shared/peer_nodes.json
export TOPOS_HOME=$TOPOS_HOME

mkdir -p $TOPOS_HOME/node/test

install_polygon() {
    # Download the polygon-edge binary
    [ ! -f "/usr/local/bin/polygon-edge" ] && $TOPOS_BIN setup subnet --path /usr/local/bin --release v${TOPOS_EDGE_VERSION}
}

link_genesis() {
    mkdir -p $TOPOS_HOME/subnet/topos
    [ ! -f "$TOPOS_HOME/subnet/topos/genesis.json" ] && ln -s /tmp/shared/genesis.json $TOPOS_HOME/subnet/topos/genesis.json
}

case "$1" in
    # Try to initialize the shared folder with predefined values
    "init")
        if [[ ! ${LOCAL_TEST_NET:-"false"} == "true" ]]; then
            echo "This command shouldn't be called for other behaviour than test"
            exit 1
        fi

        # Create and move to the shared folder
        mkdir -p /tmp/shared
        cd /tmp/shared

        if [ -f genesis.json ]; then
            echo "Configuration already created"
            exit 0
        fi

        install_polygon

        # Create nodes folder based on the expected number of validators
        polygon-edge secrets init --insecure --data-dir node- --num ${VALIDATOR_COUNT}

        # Create the guard file for pick concurrency
        touch /tmp/shared/guard.lock

        # Get the boot node peer id
        BOOT_NODE_ID=$(polygon-edge secrets output --data-dir node-1 | grep Node | head -n 1 | awk -F ' ' '{print $4}')

        # Create the bootnode multiaddr
        BOOT_NODE=$BOOT_NODE/$BOOT_NODE_ID

        # Generate genesis file
        polygon-edge genesis --consensus ibft --ibft-validators-prefix-path node- --bootnode $BOOT_NODE

        mv ./node-1 boot

        # Update permissions on the shared tree
        chmod a+rwx /tmp/shared/*
    ;;
    "boot")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then

           echo "Generating node list file..."
           $JQ -n --arg NODE $NODE '{"nodes": [$NODE]}' > $NODE_LIST_PATH
           echo "Peer nodes list have been successfully generated"

           cp -R /tmp/shared/boot/* $TOPOS_HOME/node/test

           link_genesis
       fi

        exec "$TOPOS_BIN" "${@:2}"
    ;;
    "peer")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then

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

           # The init container should have produce a bunch of config folder
           # A peer will take one and rename it to its hostname
           # If a folder with its hostname already exists we just reuse it
           # If not we process to lock the guard and reserve a slot
           EXPECTED_FOLDER="/tmp/shared/$HOSTNAME"

           if [ ! -d $EXPECTED_FOLDER ]; then

               # Acquire lock for config
               (
                   flock --exclusive -w 10 200 || exit 1


                   NEXT_NODE_FOLDER=$(ls -d /tmp/shared/node-*/ | head -n1)
                   if [[ $NEXT_NODE_FOLDER == "" ]]; then
                       echo "No more configuration available for node, Try to increase VALIDATOR_COUNT"
                       exit 1;
                   fi

                   echo "PEER has select : $NEXT_NODE_FOLDER"

                   mv $NEXT_NODE_FOLDER $EXPECTED_FOLDER
               ) 200>"/tmp/shared/guard.lock"
           fi

           cp -R $EXPECTED_FOLDER/* $TOPOS_HOME/node/test

           link_genesis
       fi

       exec "$TOPOS_BIN" "${@:2}"
       ;;

   "sync")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then

           link_genesis

           cd $TOPOS_HOME/node/test

           if [ ! -d "./libp2p" ]; then
               install_polygon

               polygon-edge secrets init --insecure --data-dir .
           fi

       fi

       exec "$TOPOS_BIN" "${@:2}"
       ;;


   *)
       exec "$TOPOS_BIN" "$@"
       ;;
esac


