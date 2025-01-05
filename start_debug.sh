#!/bin/bash

# define a environment variable DOKA_PRJ_FOLDER : /home/denis/wks-doka-one
ROOT_FOLDER="$DOKA_PRJ_FOLDER/doka.one/target/debug"
CLUSTER_PROFILE="test_03"

./start_services.sh $ROOT_FOLDER $CLUSTER_PROFILE