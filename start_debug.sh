#!/bin/bash

# define a environment variable DOKA_PRJ_FOLDER : /home/denis/wks-doka-one
export ROOT_FOLDER="$DOKA_PRJ_FOLDER/doka.one/target/debug"
export CLUSTER_PROFILE="dev_02"

./start_services.sh $ROOT_FOLDER $CLUSTER_PROFILE
