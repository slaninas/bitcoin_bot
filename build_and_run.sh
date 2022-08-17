#!/bin/bash

NAME=bitcoin_bot
LAST_BLOCK=$1

docker build -t $NAME --build-arg LAST_BLOCK=$LAST_BLOCK . && \
    docker run --rm -ti --name $NAME $NAME
