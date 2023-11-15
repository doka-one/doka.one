#!/bin/bash

# define a environment variable DOKA_PRJ_FOLDER : C:\Users\denis\wks-doka-one
ROOT_FOLDER="$DOKA_PRJ_FOLDER/doka.one/target/debug"

echo *************************
echo ***** KEY MANAGER *******
echo *************************
gnome-terminal --title="key manager" -- $ROOT_FOLDER/key-manager &

echo **************************
echo ***** SESSION MANAGER ****
echo **************************
gnome-terminal --title="session manager" -- $ROOT_FOLDER/session-manager &

echo **************************
echo ***** ADMIN SERVER *******
echo **************************
gnome-terminal --title="admin manager" -- $ROOT_FOLDER/admin-server &

echo *****************************
echo ***** DOCUMENT SERVER *******
echo *****************************
gnome-terminal --title="document manager" -- $ROOT_FOLDER/document-server &

echo *****************************
echo ***** FILE SERVER *******
echo *****************************
gnome-terminal --title="file server" -- $ROOT_FOLDER/file-server &

echo *****************************
echo ***** TIKA SERVER *******
echo *****************************
gnome-terminal --title="tika server" -- java -jar $DOKA_PRJ_FOLDER/tika/tika-server-standard-2.2.0.jar --port 40010 &
