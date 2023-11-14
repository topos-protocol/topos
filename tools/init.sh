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
NODE="http://$HOSTNAME:1340"
TCE_EXT_HOST="/dns4/$HOSTNAME"
FIXED_BOOT_PEER_ID="BOOT_NODE_1"
export TOPOS_HOME=$TOPOS_HOME

mkdir -p $TOPOS_HOME/node/test/libp2p
mkdir -p $TOPOS_HOME/node/test/consensus

random_entry () {
    local seed=$(hostname -i | tr -d ".")
    local result=$(jq -r --arg i $(($seed % $(jq '.keys|length' $1))) '.keys[$i|tonumber]' $1)

    echo $result
}

case "$1" in
    "boot")
       exec "$TOPOS_BIN" "${@:2}"
    ;;
   "peer")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then
           export TOPOS_HOME=$TOPOS_HOME
           export TCE_LOCAL_KS=$HOSTNAME
           export TCE_EXT_HOST

           LIBP2P_KEY=$(random_entry /tmp/shared/libp2p_keys.json)
           echo -n $LIBP2P_KEY > $TOPOS_HOME/node/test/libp2p/libp2p.key

           VALIDATOR_BLS_KEY=$(random_entry /tmp/shared/validator_bls_keys.json)
           echo -n $VALIDATOR_BLS_KEY > $TOPOS_HOME/node/test/consensus/validator-bls.key

           VALIDATOR_KEY=$(random_entry /tmp/shared/validator_keys.json)
           echo -n $VALIDATOR_KEY > $TOPOS_HOME/node/test/consensus/validator.key

       fi

       exec "$TOPOS_BIN" "${@:2}"
       ;;

   "sync")
       if [[ ${LOCAL_TEST_NET:-"false"} == "true" ]]; then

           export TCE_LOCAL_KS=$HOSTNAME
           export TCE_EXT_HOST

           LIBP2P_KEY=$(random_entry /tmp/shared/libp2p_keys.json)
           echo -n $LIBP2P_KEY > $TOPOS_HOME/node/test/libp2p/libp2p.key

           VALIDATOR_BLS_KEY=$(random_entry /tmp/shared/validator_bls_keys.json)
           echo -n $VALIDATOR_BLS_KEY > $TOPOS_HOME/node/test/consensus/validator-bls.key

           VALIDATOR_KEY=$(random_entry /tmp/shared/validator_keys.json)
           echo -n $VALIDATOR_KEY > $TOPOS_HOME/node/test/consensus/validator.key

       fi

       exec "$TOPOS_BIN" "${@:2}"
       ;;


   *)
       exec "$TOPOS_BIN" "$@"
       ;;
esac
