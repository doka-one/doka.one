
# define a environment variable DOKA_PRJ_FOLDER : C:\Users\denis\wks-doka-one
ROOT_FOLDER="$DOKA_PRJ_FOLDER/doka.one/target/debug"

echo *****************************
echo ***** TIKA SERVER *******
echo *****************************
gnome-terminal --title="tika server" -- java -jar $DOKA_PRJ_FOLDER/tika/tika-server-standard-2.2.0.jar --port 40010 &
