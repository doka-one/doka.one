#!/bin/bash

ROOT_FOLDER=$1
CLUSTER_PROFILE=$2

echo "root folder: $ROOT_FOLDER"
echo "cluster profile: $CLUSTER_PROFILE"


echo *************************
echo ***** KEY MANAGER *******
echo *************************
echo ">>>> gnome-terminal --title=\"key manager\" -- $ROOT_FOLDER/key-manager --cluster-profile $CLUSTER_PROFILE"
gnome-terminal --title="key manager" -- $ROOT_FOLDER/key-manager --cluster-profile $CLUSTER_PROFILE &

echo **************************
echo ***** SESSION MANAGER ****
echo **************************
echo ">>>> gnome-terminal --title=\"session manager\" -- $ROOT_FOLDER/session-manager --cluster-profile $CLUSTER_PROFILE"
gnome-terminal --title="session manager" -- $ROOT_FOLDER/session-manager --cluster-profile $CLUSTER_PROFILE &

echo **************************
echo ***** ADMIN SERVER *******
echo **************************

gnome-terminal --title="admin manager" -- $ROOT_FOLDER/admin-server --cluster-profile $CLUSTER_PROFILE &

echo *****************************
echo ***** DOCUMENT SERVER *******
echo *****************************
gnome-terminal --title="document server" -- $ROOT_FOLDER/document-server --cluster-profile $CLUSTER_PROFILE &

echo *****************************
echo ***** FILE SERVER *******
echo *****************************
gnome-terminal --title="file server" -- $ROOT_FOLDER/file-server --cluster-profile $CLUSTER_PROFILE &

echo *****************************
echo ***** TIKA SERVER *******
echo *****************************
gnome-terminal --title="tika server" -- java -jar $DOKA_PRJ_FOLDER/tika/tika-server-standard-2.2.0.jar --port 40010 &

echo *****************************
echo ***** HARBOR ****************
echo *****************************
gnome-terminal --title="harbor" -- $ROOT_FOLDER/doka-harbor --cluster-profile $CLUSTER_PROFILE &