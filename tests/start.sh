#!/usr/bin/env bash

set -eux

echo "Getting primary node from 'http://${BOOTSTRAP_SERVICE_HOST}:8000'"

SELF=$(hostname -I | tr -d ' ')
FIRST=$(curl -d ${SELF} http://${BOOTSTRAP_SERVICE_HOST}:8000)

echo "Got first=${FIRST}"

if [[ "${FIRST}" = "${SELF}" ]]
then
    echo "I am the primary"

    sawadm keygen
    sawset genesis \
        -k /etc/sawtooth/keys/validator.priv \
        -o config-genesis.batch
    sawset proposal create \
        -k /etc/sawtooth/keys/validator.priv \
        sawtooth.consensus.algorithm=None \
        sawtooth.consensus.pbft.peers=[] \
        sawtooth.consensus.pbft.block_duration=100 \
        sawtooth.consensus.pbft.checkpoint_period=10 \
        sawtooth.consensus.pbft.view_change_timeout=30 \
        sawtooth.consensus.pbft.message_timeout=10 \
        sawtooth.consensus.pbft.max_log_size=1000 \
       -o config.batch
    sawadm genesis \
        config-genesis.batch config.batch
    sawtooth-validator -vv \
        --endpoint tcp://${SELF}:8800 \
        --bind component:tcp://eth0:4004 \
        --bind network:tcp://eth0:8800 \
        --bind consensus:tcp://eth0:5050 \
        --peering dynamic
else
    echo "I am the secondary :("

    sawadm keygen
    sawtooth keygen my_key
    sawtooth-validator -vv \
        --endpoint tcp://${SELF}:8800 \
        --bind component:tcp://eth0:4004 \
        --bind network:tcp://eth0:8800 \
        --bind consensus:tcp://eth0:5050 \
        --peering dynamic \
        --seeds tcp://${FIRST}:8800
fi