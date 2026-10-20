#!/bin/bash

# define an environment variable DOKA_PRJ_FOLDER : /home/denis/wks-doka-one
export ROOT_FOLDER="$DOKA_PRJ_FOLDER/doka.one/target/debug"
export TIKA_JAR="$DOKA_PRJ_FOLDER/tika/tika-server-standard-2.2.0.jar"
export JAVA_EXE="java"
export CLUSTER_PROFILE="dev_02"

echo *************************
echo ***** KEY MANAGER *******
echo *************************
gnome-terminal --title="key-manager" -- "$ROOT_FOLDER/key-manager" --cluster-profile "$CLUSTER_PROFILE" &

echo **************************
echo ***** SESSION MANAGER ****
echo **************************
gnome-terminal --title="session-manager" -- "$ROOT_FOLDER/session-manager" --cluster-profile "$CLUSTER_PROFILE" &

echo **************************
echo ***** ADMIN SERVER *******
echo **************************
gnome-terminal --title="admin-server" -- "$ROOT_FOLDER/admin-server" --cluster-profile "$CLUSTER_PROFILE" &

echo *****************************
echo ***** DOCUMENT SERVER *******
echo *****************************
gnome-terminal --title="document-server" -- "$ROOT_FOLDER/document-server" --cluster-profile "$CLUSTER_PROFILE" &

echo *****************************
echo ***** FILE SERVER *******
echo *****************************
gnome-terminal --title="file-server" -- "$ROOT_FOLDER/file-server" --cluster-profile "$CLUSTER_PROFILE" &

echo *****************************
echo ***** TIKA SERVER *******
echo *****************************
gnome-terminal --title="tika-server" -- "$JAVA_EXE" -jar "$TIKA_JAR" --port 40010 &
