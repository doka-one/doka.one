#!/bin/bash

## Send the artefacts to the website

read -p "Enter the web server hostname (ex : doka.one) : " SERVER_NAME
read -p "Enter the user name for the web server machine (ex: root) : " USER_NAME
read -s -p "Enter the password for the user of the web server machine (ex: iiss...) : " PASS

clear

DOKA_SERVICE_SOURCE_FOLDER='/mnt/c/Users/denis/.cargo/bin'
PUBLIC_SOURCE_FOLDER='/mnt/c/Users/denis/Dropbox/public/0.1.0'
TARGET_FOLDER="$USER_NAME@$SERVER_NAME:/$USER_NAME/doka.one/content/artefacts/0.1.0/"

echo "Upload key-manager"
rm $DOKA_SERVICE_SOURCE_FOLDER/key-manager.zip
# zip -r key-manager.zip key-manager.exe
cd $DOKA_SERVICE_SOURCE_FOLDER
zip $DOKA_SERVICE_SOURCE_FOLDER/key-manager.zip  ./key-manager.exe
# gzip -c $DOKA_SERVICE_SOURCE_FOLDER/key-manager.exe > $DOKA_SERVICE_SOURCE_FOLDER/key-manager.zip
sshpass -p $PASS scp $DOKA_SERVICE_SOURCE_FOLDER/key-manager.zip $TARGET_FOLDER
echo "Done"

echo "Upload session-manager"
rm $DOKA_SERVICE_SOURCE_FOLDER/session-manager.zip
cd $DOKA_SERVICE_SOURCE_FOLDER
zip $DOKA_SERVICE_SOURCE_FOLDER/session-manager.zip  ./session-manager.exe
# gzip -c $DOKA_SERVICE_SOURCE_FOLDER/session-manager.exe > $DOKA_SERVICE_SOURCE_FOLDER/session-manager.zip
sshpass -p $PASS scp $DOKA_SERVICE_SOURCE_FOLDER/session-manager.zip $TARGET_FOLDER
echo "Done"

# ----

if [ "$1" == "ALL" ]; then

  echo "Upload tika-server"
  sshpass -p $PASS scp $PUBLIC_SOURCE_FOLDER/tika-server.zip $TARGET_FOLDER
  echo "Done"

  echo "Upload jdk-17"
  sshpass -p $PASS scp $PUBLIC_SOURCE_FOLDER/jdk-17.zip $TARGET_FOLDER
  echo "Done"

  echo "Upload serman"
  sshpass -p $PASS scp $PUBLIC_SOURCE_FOLDER/serman.zip $TARGET_FOLDER
  echo "Done"

fi

# ----
echo "Build the web site container"
sshpass -p $PASS ssh -t $USER_NAME@$SERVER_NAME  "cd doka.one;python3 make.py"
echo "Done"