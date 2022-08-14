#!/bin/bash

NAME=bitcoin_bot
docker build -t $NAME . && \
    docker run --rm -ti --name $NAME $NAME
